# Native `this` expression contexts

TypeScript 7.0.2 native hook and semantic API independently agreed on the summary
and error-type identity of all 23 roots under both strict and loose profiles.
The initial capture (`vrun-68391da0-ffea-4273-a7e8-e290743d0110`) completed those
comparisons and failed only because the baseline files had not yet been saved.
The expected files are the unmodified captures from that run.

The cases distinguish module/script scope, lexical arrows, ordinary and explicit
function `this`, class instance/static methods, parameter defaults and patterns,
object contextual `ThisType`, nested object context, and constructor pre-super
checks. Diagnostics 2683/7041/2331/17009 do not make these roots native errorType.
The annotated `contextual` root proves its declared type only; it does not prove
that serialization evaluated the function body's `this` expression. Final query
tests use depth zero to isolate acquisition and summary from member expansion.
