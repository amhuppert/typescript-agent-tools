//! npm-style version ranges, as TypeScript evaluates `typesVersions` keys and
//! `types@<range>` export conditions against its own version, and the
//! version a package manifest declares.

use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Version {
    major: u64,
    minor: u64,
    patch: u64,
    prerelease: Vec<String>,
}

impl Version {
    /// A package manifest's `version`, as npm's `semver.valid` reads one
    /// (SemVer 2.0.0): three numeric components without leading zeros, then
    /// an optional `-` prerelease of dot-separated identifiers, each numeric
    /// without a leading zero or alphanumeric with hyphens, and an optional
    /// `+` build of dot-separated alphanumeric identifiers, which no
    /// comparison reads. Anything else, `5.6`, `v5.6.3`, or a component past
    /// `u64::MAX` included, is no version.
    pub(super) fn parse(text: &str) -> Option<Self> {
        let identifier = |part: &str| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        };
        let numeric = |part: &str| {
            !part.is_empty()
                && part.bytes().all(|byte| byte.is_ascii_digit())
                && (part == "0" || !part.starts_with('0'))
        };
        let (rest, build) = match text.split_once('+') {
            Some((rest, build)) => (rest, Some(build)),
            None => (text, None),
        };
        if build.is_some_and(|build| !build.split('.').all(identifier)) {
            return None;
        }
        let (core, prerelease) = match rest.split_once('-') {
            Some((core, prerelease)) => (core, Some(prerelease)),
            None => (rest, None),
        };
        let prerelease = match prerelease {
            None => Vec::new(),
            Some(prerelease) => {
                let parts: Vec<&str> = prerelease.split('.').collect();
                let valid = |part: &&str| {
                    identifier(part)
                        && (!part.bytes().all(|byte| byte.is_ascii_digit()) || numeric(part))
                };
                if !parts.iter().all(valid) {
                    return None;
                }
                parts.into_iter().map(str::to_owned).collect()
            }
        };
        let components: Vec<&str> = core.split('.').collect();
        let [major, minor, patch] = components.as_slice() else {
            return None;
        };
        let number = |part: &str| numeric(part).then(|| part.parse::<u64>().ok()).flatten();
        Some(Self {
            major: number(major)?,
            minor: number(minor)?,
            patch: number(patch)?,
            prerelease,
        })
    }

    /// The major component, which selects a compiler's dialect.
    pub(super) fn major(&self) -> u64 {
        self.major
    }
}

/// The pinned reference compiler's version, the semantic authority of
/// every dialect.
pub(super) const REFERENCE: &str = "7.0.2";

impl Version {
    /// `REFERENCE`.
    pub(super) fn reference() -> Self {
        Self::parse(REFERENCE).expect("the reference version is a version")
    }
}

/// Whether `range` admits `version` (`VersionRange.test`), as a compiler
/// evaluates a `typesVersions` key or a `types@` condition against its own
/// version, the dialect's (`compatibility::Dialect::typescript`). A range
/// that does not parse matches nothing.
pub(super) fn matches_typescript(range: &str, version: &Version) -> bool {
    parse_range(range).is_some_and(|alternatives| {
        alternatives.is_empty()
            || alternatives.iter().any(|comparators| {
                comparators
                    .iter()
                    .all(|(operator, operand)| operator.test(version.compare(operand)))
            })
    })
}

#[derive(Debug, Clone, Copy)]
enum Operator {
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Equal,
}

impl Operator {
    fn test(self, ordering: Ordering) -> bool {
        match self {
            Operator::Less => ordering.is_lt(),
            Operator::LessEqual => ordering.is_le(),
            Operator::Greater => ordering.is_gt(),
            Operator::GreaterEqual => ordering.is_ge(),
            Operator::Equal => ordering.is_eq(),
        }
    }
}

type Comparator = (Operator, Version);

impl Version {
    fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
            prerelease: Vec::new(),
        }
    }

    fn with_zero_prerelease(mut self) -> Self {
        self.prerelease = vec!["0".into()];
        self
    }

    fn increment(&self, part: Part) -> Self {
        match part {
            // Ranges come from package metadata; a bound past `u64::MAX`
            // saturates, which no real version reaches.
            Part::Major => Self::new(self.major.saturating_add(1), 0, 0),
            Part::Minor => Self::new(self.major, self.minor.saturating_add(1), 0),
            Part::Patch => Self::new(self.major, self.minor, self.patch.saturating_add(1)),
        }
    }

    fn compare(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| compare_prerelease(&self.prerelease, &other.prerelease))
    }
}

/// A release outranks its prereleases; identifiers compare numerically when
/// both are numeric, and numeric identifiers rank below alphanumeric ones.
fn compare_prerelease(left: &[String], right: &[String]) -> Ordering {
    match (left.is_empty(), right.is_empty()) {
        (true, true) => return Ordering::Equal,
        (true, false) => return Ordering::Greater,
        (false, true) => return Ordering::Less,
        _ => {}
    }
    for (left, right) in left.iter().zip(right) {
        let numeric = |text: &str| !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
        let ordering = match (numeric(left), numeric(right)) {
            (true, true) => left
                .parse::<u64>()
                .unwrap_or(u64::MAX)
                .cmp(&right.parse::<u64>().unwrap_or(u64::MAX)),
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => left.cmp(right),
        };
        if ordering.is_ne() {
            return ordering;
        }
    }
    left.len().cmp(&right.len())
}

#[derive(Clone, Copy)]
enum Part {
    Major,
    Minor,
    Patch,
}

/// A version with possibly wildcard components.
struct Partial {
    version: Version,
    major: bool,
    minor: bool,
    patch: bool,
}

fn is_wildcard(part: &str) -> bool {
    matches!(part, "*" | "x" | "X")
}

fn parse_partial(text: &str) -> Option<Partial> {
    let (core, prerelease) = match text.split_once('-') {
        Some((core, rest)) => (
            core,
            Some(rest.split_once('+').map_or(rest, |(pre, _)| pre)),
        ),
        None => (text.split_once('+').map_or(text, |(core, _)| core), None),
    };
    let parts: Vec<&str> = core.split('.').collect();
    if parts.is_empty() || parts.len() > 3 || (prerelease.is_some() && parts.len() < 3) {
        return None;
    }
    let valid = |part: &str| {
        is_wildcard(part)
            || part == "0"
            || (part.bytes().all(|b| b.is_ascii_digit())
                && !part.is_empty()
                && !part.starts_with('0'))
    };
    if !parts.iter().all(|part| valid(part)) {
        return None;
    }
    let prerelease: Vec<String> = match prerelease {
        Some(text) => {
            if text.is_empty()
                || !text
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
            {
                return None;
            }
            text.split('.').map(str::to_owned).collect()
        }
        None => Vec::new(),
    };
    let major = is_wildcard(parts[0]);
    let minor = major || parts.get(1).is_none_or(|part| is_wildcard(part));
    let patch = minor || parts.get(2).is_none_or(|part| is_wildcard(part));
    let number = |index: usize, wildcard: bool| {
        if wildcard {
            0
        } else {
            parts[index].parse().unwrap_or(u64::MAX)
        }
    };
    Some(Partial {
        version: Version {
            major: number(0, major),
            minor: number(1, minor),
            patch: number(2, patch),
            prerelease,
        },
        major,
        minor,
        patch,
    })
}

fn parse_range(text: &str) -> Option<Vec<Vec<Comparator>>> {
    let mut alternatives = Vec::new();
    for range in text.trim().split("||") {
        let range = range.trim();
        if range.is_empty() {
            continue;
        }
        let mut comparators = Vec::new();
        let words: Vec<&str> = range.split_whitespace().collect();
        if words.len() == 3 && words[1] == "-" {
            parse_hyphen(words[0], words[2], &mut comparators)?;
        } else {
            for word in words {
                parse_comparator(word, &mut comparators)?;
            }
        }
        alternatives.push(comparators);
    }
    Some(alternatives)
}

fn parse_hyphen(left: &str, right: &str, comparators: &mut Vec<Comparator>) -> Option<()> {
    let left = parse_partial(left)?;
    let right = parse_partial(right)?;
    if !left.major {
        comparators.push((Operator::GreaterEqual, left.version));
    }
    if !right.major {
        comparators.push(if right.minor {
            (Operator::Less, right.version.increment(Part::Major))
        } else if right.patch {
            (Operator::Less, right.version.increment(Part::Minor))
        } else {
            (Operator::LessEqual, right.version)
        });
    }
    Some(())
}

fn parse_comparator(text: &str, comparators: &mut Vec<Comparator>) -> Option<()> {
    let (operator, rest) = ["<=", ">=", "~", "^", "<", ">", "="]
        .iter()
        .find_map(|operator| text.strip_prefix(operator).map(|rest| (*operator, rest)))
        .unwrap_or(("", text));
    let partial = parse_partial(rest)?;
    let version = &partial.version;
    if partial.major {
        if matches!(operator, "<" | ">") {
            comparators.push((Operator::Less, Version::new(0, 0, 0)));
        }
        return Some(());
    }
    match operator {
        "~" => {
            comparators.push((Operator::GreaterEqual, version.clone()));
            let part = if partial.minor {
                Part::Major
            } else {
                Part::Minor
            };
            comparators.push((Operator::Less, version.increment(part)));
        }
        "^" => {
            comparators.push((Operator::GreaterEqual, version.clone()));
            let part = if version.major > 0 || partial.minor {
                Part::Major
            } else if version.minor > 0 || partial.patch {
                Part::Minor
            } else {
                Part::Patch
            };
            comparators.push((Operator::Less, version.increment(part)));
        }
        "<" | ">=" => {
            let operator = if operator == "<" {
                Operator::Less
            } else {
                Operator::GreaterEqual
            };
            let operand = if partial.minor || partial.patch {
                version.clone().with_zero_prerelease()
            } else {
                version.clone()
            };
            comparators.push((operator, operand));
        }
        "<=" | ">" => {
            let wildcard = if partial.minor {
                Some(Part::Major)
            } else if partial.patch {
                Some(Part::Minor)
            } else {
                None
            };
            comparators.push(match (wildcard, operator == "<=") {
                (Some(part), true) => (
                    Operator::Less,
                    version.increment(part).with_zero_prerelease(),
                ),
                (Some(part), false) => (
                    Operator::GreaterEqual,
                    version.increment(part).with_zero_prerelease(),
                ),
                (None, true) => (Operator::LessEqual, version.clone()),
                (None, false) => (Operator::Greater, version.clone()),
            });
        }
        _ => {
            if partial.minor || partial.patch {
                let part = if partial.minor {
                    Part::Major
                } else {
                    Part::Minor
                };
                comparators.push((
                    Operator::GreaterEqual,
                    version.clone().with_zero_prerelease(),
                ));
                comparators.push((
                    Operator::Less,
                    version.increment(part).with_zero_prerelease(),
                ));
            } else {
                comparators.push((Operator::Equal, version.clone()));
            }
        }
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::{REFERENCE, Version, matches_typescript};

    #[test]
    fn a_manifest_version_is_a_full_semver_version() {
        for (text, major) in [
            ("5.6.3", Some(5)),
            ("0.0.0", Some(0)),
            ("7.0.2", Some(7)),
            ("5.7.0-beta", Some(5)),
            ("5.7.0-dev.20241001", Some(5)),
            ("5.7.0-rc.1+sha.abc-1", Some(5)),
            ("5.6.3+build", Some(5)),
            ("5.6", None),
            ("5", None),
            ("v5.6.3", None),
            ("5.6.3.1", None),
            ("05.6.3", None),
            ("5.06.3", None),
            ("5.6.3-", None),
            ("5.6.3-01", None),
            ("5.6.3-a..b", None),
            ("5.6.3+", None),
            ("5.6.3+a_b", None),
            (" 5.6.3", None),
            ("latest", None),
            ("", None),
            ("99999999999999999999.0.0", None),
        ] {
            assert_eq!(Version::parse(text).map(|v| v.major()), major, "{text:?}");
        }
    }

    #[test]
    fn evaluates_ranges_against_the_reference_compiler() {
        let version = Version::parse(REFERENCE).unwrap();
        for (range, expected) in [
            ("*", true),
            ("", true),
            (">=4.0", true),
            (">=7.1", false),
            ("<7", false),
            ("<7.1", true),
            ("<=7.0", true),
            (">7.0", false),
            (">7.0.1", true),
            ("7", true),
            ("7.x", true),
            ("6.x || 7.0.x", true),
            ("~7.0.1", true),
            ("^6.5", false),
            ("^7.0.0-beta", true),
            ("4.0 - 7.0.1", false),
            ("4.0 - 7", true),
            (">=3.1 <8", true),
            (">= 3.1", false),
            ("=7.0.2", true),
            ("<7.0.2-0", false),
            ("not a range", false),
            ("01.2", false),
        ] {
            assert_eq!(matches_typescript(range, &version), expected, "{range}");
        }
    }

    /// A range is evaluated against the version given, at, below and above
    /// its bounds; 5.6.3's `VersionRange.test` agrees on 5.6.3.
    #[test]
    fn evaluates_ranges_against_the_version_given() {
        for (version, expected) in [
            ("5.6.3", true),
            ("5.6.2", false),
            ("5.7.0", false),
            ("5.6.4-beta", true),
            ("7.0.2", false),
        ] {
            let version = Version::parse(version).unwrap();
            assert_eq!(
                matches_typescript(">=5.6.3 <5.7", &version),
                expected,
                "{version:?}"
            );
        }
        let typescript = Version::parse("5.6.3").unwrap();
        for (range, expected) in [("<6", true), ("5.x", true), ("^5.4", true), (">=7", false)] {
            assert_eq!(matches_typescript(range, &typescript), expected, "{range}");
        }
    }
}
