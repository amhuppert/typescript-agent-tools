//! TypeScript's single-`*` module name patterns, shared by `paths` mappings
//! and pattern ambient modules such as `declare module "*.css"`.

/// The text before and after a pattern's one `*`, as TypeScript's
/// `tryParsePattern` splits it. `None` for text without a `*`, or with more
/// than one, which TypeScript rejects (TS5061) and never matches.
pub(super) fn parse(pattern: &str) -> Option<(&str, &str)> {
    let (prefix, suffix) = pattern.split_once('*')?;
    (!suffix.contains('*')).then_some((prefix, suffix))
}

/// TypeScript's `findBestPatternMatch`: of the `(pattern, value)` pairs
/// whose pattern matches `name`, the one with the longest prefix, the
/// earliest on a tie, with the text its `*` matched.
pub(super) fn best_match<'p, 'n, T>(
    patterns: impl IntoIterator<Item = (&'p str, T)>,
    name: &'n str,
) -> Option<(T, &'n str)> {
    let mut best: Option<(T, &'p str, &'p str)> = None;
    for (pattern, value) in patterns {
        let Some((prefix, suffix)) = parse(pattern) else {
            continue;
        };
        if name.len() >= prefix.len() + suffix.len()
            && name.starts_with(prefix)
            && name.ends_with(suffix)
            && best
                .as_ref()
                .is_none_or(|(_, longest, _)| prefix.len() > longest.len())
        {
            best = Some((value, prefix, suffix));
        }
    }
    let (value, prefix, suffix) = best?;
    Some((value, &name[prefix.len()..name.len() - suffix.len()]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_longest_prefix_wins_and_the_earliest_breaks_a_tie() {
        let patterns = [("*.css", 0), ("*.module.css", 1), ("app*", 2), ("*s", 3)];
        assert_eq!(
            best_match(patterns, "app.module.css"),
            Some((2, ".module.css"))
        );
        assert_eq!(best_match(patterns, "x.module.css"), Some((0, "x.module")));
        assert_eq!(best_match([("a*b*c", 0)], "aXbYc"), None);
        assert_eq!(parse("a*b*c"), None);
        assert_eq!(parse("exact"), None);
    }
}
