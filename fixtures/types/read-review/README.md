# Fresh identifier-read observations

`answers.json` contains twelve independent TypeScript 7.0.2 API and
same-checker Go hook observations. It preserves full requests, captured input
hashes and demand-phase diagnostics. `provenance.json` identifies the live
source snapshot reviewed afterward; subsequent root fixes are separate.

The historical probe was run from `.cc/temp/types-reader-review/probe.mjs`.
Its relative imports and scratch paths preserve that layout. For reproduction,
copy the captured probe back to that scratch path and provide the pinned
`TYPE_SCRIPT_SEMANTIC_API` and `TYPE_SCRIPT_NATIVE` environment variables. It
does not invoke whole-program semantic diagnostics.

See [native read evidence](../../../docs/types-native-read-evidence.md) for scope, the exact used-before-assignment
branch and the distinct unrelated-condition/never-antecedent observations.
