These thirty TypeScript 7.0.2 hook/API queries distinguish installed global
identity observations from factories that need a usable global type. Healthy,
absent, wrong-arity and wrong-kind globals leave unrelated generic references
and contextual `this` unchanged. Array-only and broken-ReadonlyArray profiles
establish the native fallback to the actual Array identity.

Initial twenty-row capture `vrun-616d29f7-03e2-4894-affa-eef87ebe6697` and expanded
capture `vrun-415a277c-2719-4393-af27-b668d50e66ec` matched the ordinary API on
every row; their only assertion failures were the not-yet-installed baseline.
The checked-in baseline is the exact expanded capture.

Native array recovery displays `{}` and is not `errorType`. The design's consumed
global-recovery contract makes those actual factory queries semantic failures.
That product distinction is asserted separately from ordinary identity reads in
`tests/type_fixed_globals.rs`; diagnostic text alone is not the classifier.
