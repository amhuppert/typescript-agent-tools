# Normalized parameter-name copy isolation

Native interface merging permits a later declaration to add a defaulted type
parameter. Its existing member with the same name supplies the first own
written spelling; the parameter declaration supplies the normalized name.

The cheap member has a one-character name and a padded comment inside its
computed name. The chosen member repeats the same character N times. Written
names therefore have equal growth in characters and UTF-8 bytes (with a fixed
scaffolding difference), while only the chosen normalized parameter name grows.
The first token of each computed name is a single bracket, so reading its span
does not introduce a second width-dependent identifier scan.

ASCII, BMP, supplementary, escaped ASCII, escaped supplementary and underscore
variants produce clean number queries at widths 16 and 64 in both noCheck states.
Separate native selections observe the exact first-own written names.
This fixture is intended to audit normalized-name accounting after written-name
accounting was added; omission sensitivity still requires the Rust experiment.
