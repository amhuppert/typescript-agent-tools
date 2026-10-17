# Native inline member display observations

`research-answers.json` preserves twelve fresh public API questions and twelve
fresh same-checker hook questions over three option states.
`research-child-answers.json` preserves three fresh actual setter-parameter
questions. The raw
answers include before/after TypeToString diagnostic phases; limit events do
not claim ordinary error-type causality. `research-provenance.json` records
identical six tooling/hook hashes before and after the exploratory capture.

The formal suite `tools/type-member-display-oracle.test.mjs` passed four tests,
zero skips, in `vrun-06824310-0f87-4728-aa41-94cfbc8ccf10`. The raw experiments
precede that registered run; this archive does not label them as that exact run.
Current exact tools matching the exploratory hashes are copied under
`captured-tooling`. Additional module dependencies and the formal suite are
copied separately without retroactively claiming earlier before/after hashes.

The original exploratory layout was `.cc/temp/types-member-display/`. Restore
the published parent inputs there and rename research probe/child filenames to
their original names to replay those relative imports. Shared copied tooling
expects the original `tools` and `fixtures/types/compiler-hook` layout. It also
requires the pinned installed compiler API/binary and clean native source at
commit `2bd066d87f5bafd315be9f40889d0a60b9e58e0b`; this is historical research
evidence rather than another live oracle or a self-contained compiler bundle.

Each hook answer records the exact captured filesystem inputHashes. Public API
answers record selected source hashes but did not capture every compiler-read
metadata/library input. Copied tooling cannot retroactively fill that provenance
gap. Production output and arbitrary root-causal recovery, compiler counters,
cache protocols and resource calibration are separate obligations.
