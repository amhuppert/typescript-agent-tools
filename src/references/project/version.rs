//! npm-style version ranges, as TypeScript evaluates `typesVersions` keys and
//! `types@<range>` export conditions against its own version.

use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Version {
    major: u64,
    minor: u64,
    patch: u64,
    prerelease: Vec<String>,
}

/// The pinned reference compiler's version.
fn typescript() -> Version {
    Version::new(7, 0, 2)
}

/// Whether `range` admits the reference compiler's version; `None` when the
/// range does not parse, which TypeScript treats as not matching.
pub(super) fn matches_typescript(range: &str) -> Option<bool> {
    let version = typescript();
    let alternatives = parse_range(range)?;
    Some(
        alternatives.is_empty()
            || alternatives.iter().any(|comparators| {
                comparators
                    .iter()
                    .all(|(operator, operand)| operator.test(version.compare(operand)))
            }),
    )
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
    use super::matches_typescript;

    #[test]
    fn evaluates_ranges_against_the_reference_compiler() {
        for (range, expected) in [
            ("*", Some(true)),
            ("", Some(true)),
            (">=4.0", Some(true)),
            (">=7.1", Some(false)),
            ("<7", Some(false)),
            ("<7.1", Some(true)),
            ("<=7.0", Some(true)),
            (">7.0", Some(false)),
            (">7.0.1", Some(true)),
            ("7", Some(true)),
            ("7.x", Some(true)),
            ("6.x || 7.0.x", Some(true)),
            ("~7.0.1", Some(true)),
            ("^6.5", Some(false)),
            ("^7.0.0-beta", Some(true)),
            ("4.0 - 7.0.1", Some(false)),
            ("4.0 - 7", Some(true)),
            (">=3.1 <8", Some(true)),
            (">= 3.1", None),
            ("=7.0.2", Some(true)),
            ("<7.0.2-0", Some(false)),
            ("not a range", None),
            ("01.2", None),
        ] {
            assert_eq!(matches_typescript(range), expected, "{range}");
        }
    }
}
