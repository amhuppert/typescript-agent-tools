//! Native spelling suggestions, independent of the semantic identities and
//! display formats supplied by their callers.

use std::cmp::Ordering;

/// `core.GetSpellingSuggestion`: native length gates mix source rune count
/// with candidate UTF-8 bytes. Ties use the caller's identity comparator.
pub(crate) fn suggestion<T: Copy>(
    name: &str,
    candidates: impl IntoIterator<Item = (T, String)>,
    compare: impl Fn(T, T) -> Ordering,
) -> Option<T> {
    match try_suggestion(name, candidates, |left, right| {
        Ok::<_, std::convert::Infallible>(compare(left, right))
    }) {
        Ok(result) => result,
        Err(error) => match error {},
    }
}

/// Semantic symbol comparers may acquire native ordering facts. Their
/// failures remain failures, and comparison occurs only on an actual tie.
pub(crate) fn try_suggestion<T: Copy, E>(
    name: &str,
    candidates: impl IntoIterator<Item = (T, String)>,
    compare: impl Fn(T, T) -> Result<Ordering, E>,
) -> Result<Option<T>, E> {
    try_suggestion_by(
        &mut (),
        name,
        candidates,
        |_, (_, name)| Ok(name),
        |_, (left, _), (right, _)| compare(left, right),
    )
    .map(|result| result.map(|(candidate, _)| candidate))
}

/// `getCandidateName` and `compareSymbols` run lazily inside the native
/// candidate traversal, with one context for their semantic acquisitions.
pub(crate) fn try_suggestion_by<T: Clone, E, C>(
    context: &mut C,
    name: &str,
    candidates: impl IntoIterator<Item = T>,
    mut candidate_name: impl FnMut(&mut C, T) -> Result<String, E>,
    mut compare: impl FnMut(&mut C, T, T) -> Result<Ordering, E>,
) -> Result<Option<T>, E> {
    let runes: Vec<_> = name.chars().collect();
    let maximum_difference = 2usize.max((runes.len() as f64 * 0.34) as usize);
    let mut best_distance = (runes.len() as f64 * 0.4).floor() + 0.9;
    let mut best = None;
    for candidate in candidates {
        let candidate_name = candidate_name(context, candidate.clone())?;
        if candidate_name.is_empty()
            || candidate_name.len().abs_diff(runes.len()) > maximum_difference
            || candidate_name == name
            || candidate_name.len() < 3 && !equal_fold(&candidate_name, name)
        {
            continue;
        }
        let candidate_runes: Vec<_> = candidate_name.chars().collect();
        let Some(distance) = levenshtein_with_max(&runes, &candidate_runes, best_distance) else {
            continue;
        };
        if distance < best_distance {
            best_distance = distance;
            best = Some(candidate);
        } else if match best {
            None => true,
            Some(ref best) => compare(context, candidate.clone(), best.clone())?.is_lt(),
        } {
            best = Some(candidate);
        }
    }
    Ok(best)
}

/// `core.levenshteinWithMax`: insertion/deletion cost one, substitution two,
/// and simple-case-only substitution one tenth. This is not a transposition
/// distance despite the explanatory comment at the native call site.
fn levenshtein_with_max(source: &[char], target: &[char], limit: f64) -> Option<f64> {
    let mut previous: Vec<_> = (0..=target.len()).map(|i| i as f64).collect();
    let mut current = vec![0.; target.len() + 1];
    let big = limit + 0.01;
    for (index, &character) in source.iter().enumerate() {
        let i = index + 1;
        let min_j = ((i as f64 - limit).ceil() as usize).max(1);
        let max_j = ((limit + i as f64).floor() as usize).min(target.len());
        let mut minimum = i as f64;
        current[0] = minimum;
        current[1..min_j.min(target.len() + 1)].fill(big);
        for j in min_j..=max_j {
            let substitution = previous[j - 1]
                + if crate::input::simple_lowercase(character)
                    == crate::input::simple_lowercase(target[j - 1])
                {
                    0.1
                } else {
                    2.
                };
            let distance = if character == target[j - 1] {
                previous[j - 1]
            } else {
                (previous[j] + 1.).min((current[j - 1] + 1.).min(substitution))
            };
            current[j] = distance;
            minimum = minimum.min(distance);
        }
        current[max_j + 1..].fill(big);
        if minimum > limit {
            return None;
        }
        std::mem::swap(&mut previous, &mut current);
    }
    (previous[target.len()] <= limit).then_some(previous[target.len()])
}

/// TypeScript 5.6.3's `getSpellingSuggestion`, which the TypeScript 5.6
/// dialect's configuration asks of an unknown option name: the nearest of
/// `candidates` by `levenshteinWithMax`, the first of equally near ones, or
/// none. Unlike TypeScript 7's rule (`suggestion`), every length counts
/// UTF-16 code units, so an astral character counts twice, and a candidate
/// shorter than three units must equal the name after JavaScript's
/// `toLowerCase`.
pub(crate) fn typescript_5_6_suggestion<'a>(
    name: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Option<&'a str> {
    let units: Vec<u16> = name.encode_utf16().collect();
    let length = units.len() as f64;
    let maximum_difference = 2f64.max((length * 0.34).floor());
    let mut best_distance = (length * 0.4).floor() + 1.;
    let lowered = crate::strings::to_lower_js(name.as_bytes());
    let mut best = None;
    for candidate in candidates {
        let candidate_units: Vec<u16> = candidate.encode_utf16().collect();
        if (candidate_units.len() as f64 - length).abs() > maximum_difference
            || candidate == name
            || candidate_units.len() < 3
                && crate::strings::to_lower_js(candidate.as_bytes()) != lowered
        {
            continue;
        }
        if let Some(distance) =
            levenshtein_with_max_utf16(&units, &candidate_units, best_distance - 0.1)
        {
            best_distance = distance;
            best = Some(candidate);
        }
    }
    best
}

/// TypeScript 5.6.3's `levenshteinWithMax` over UTF-16 code units:
/// insertion and deletion cost one, substitution two, and a substitution of
/// units that JavaScript's `toLowerCase` lowers alike, each on its own, one
/// tenth. A surrogate lowers to itself.
fn levenshtein_with_max_utf16(source: &[u16], target: &[u16], max: f64) -> Option<f64> {
    fn lower(unit: u16) -> Result<Vec<u8>, u16> {
        char::from_u32(u32::from(unit))
            .map(|c| crate::strings::to_lower_js(c.encode_utf8(&mut [0; 4]).as_bytes()))
            .ok_or(unit)
    }
    let target_lowered: Vec<_> = target.iter().map(|&unit| lower(unit)).collect();
    let mut previous: Vec<f64> = (0..=target.len()).map(|j| j as f64).collect();
    let mut current = vec![0.; target.len() + 1];
    let big = max + 0.01;
    let length = target.len() as f64;
    for (index, &unit) in source.iter().enumerate() {
        let i = (index + 1) as f64;
        let min_j = (if i > max { i - max } else { 1. }).ceil() as usize;
        let max_j = (if length > max + i { max + i } else { length }).floor() as usize;
        let unit_lowered = lower(unit);
        current[0] = i;
        let mut column_minimum = i;
        current[1..min_j.min(target.len() + 1)].fill(big);
        for j in min_j..=max_j {
            let substitution = previous[j - 1]
                + if unit_lowered == target_lowered[j - 1] {
                    0.1
                } else {
                    2.
                };
            let distance = if unit == target[j - 1] {
                previous[j - 1]
            } else {
                (previous[j] + 1.)
                    .min(current[j - 1] + 1.)
                    .min(substitution)
            };
            current[j] = distance;
            column_minimum = column_minimum.min(distance);
        }
        if max_j < target.len() {
            current[max_j + 1..].fill(big);
        }
        if column_minimum > max {
            return None;
        }
        std::mem::swap(&mut previous, &mut current);
    }
    let result = previous[target.len()];
    (result <= max).then_some(result)
}

/// Go `strings.EqualFold` compares simple Unicode fold orbits without full
/// string expansions or locale. These exceptional orbits are the pinned Go
/// Unicode 17 caseOrbit entries not already joined by simple lowercase.
fn equal_fold(left: &str, right: &str) -> bool {
    fn fold(c: char) -> char {
        if c == '\u{130}' {
            return c;
        }
        match crate::input::simple_lowercase(c) {
            '\u{17f}' => 's',
            '\u{b5}' => '\u{3bc}',
            '\u{345}' | '\u{1fbe}' => '\u{3b9}',
            '\u{3c2}' => '\u{3c3}',
            '\u{3d0}' => '\u{3b2}',
            '\u{3d1}' => '\u{3b8}',
            '\u{3d5}' => '\u{3c6}',
            '\u{3d6}' => '\u{3c0}',
            '\u{3f0}' => '\u{3ba}',
            '\u{3f1}' => '\u{3c1}',
            '\u{3f5}' => '\u{3b5}',
            '\u{1c80}' => '\u{432}',
            '\u{1c81}' => '\u{434}',
            '\u{1c82}' => '\u{43e}',
            '\u{1c83}' => '\u{441}',
            '\u{1c84}' | '\u{1c85}' => '\u{442}',
            '\u{1c86}' => '\u{44a}',
            '\u{1c87}' => '\u{463}',
            '\u{1c88}' => '\u{a64b}',
            '\u{1e9b}' => '\u{1e61}',
            '\u{1fd3}' => '\u{390}',
            '\u{1fe3}' => '\u{3b0}',
            '\u{fb05}' => '\u{fb06}',
            other => other,
        }
    }
    left.chars().map(fold).eq(right.chars().map(fold))
}
