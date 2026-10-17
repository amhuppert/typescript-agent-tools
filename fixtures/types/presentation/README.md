# Finite-presentation semantic controls

These are exact inputs for bounded fresh TypeScript 7.0.2 API observations.
Each input was written as `value.ts` beside `request-config.json` (then named
`tsconfig.json`) in a disposable directory, and opened in a new pinned API
session. `observations.json` records the demanded facts and each input hash.
The literal controls demand the `text` variable and compare its native literal
value. The object controls demand `current`, enumerate properties, and obtain
their read types. No whole-program semantic check or language-service search
was added to that history.

They establish exact values, ordered keys and shared native number identity,
not the product's 512/262,144-byte display budgets. The latter are final native
`Project::type_of` assertions in `tests/type_presentation.rs`. Its byte-boundary
key family adjusts the key length to complete documents of 262,143/262,144/
262,145 bytes; the frozen long-key control separately proves an exact large
native key. Full graph kinds, node-boundary, lazy failure and resource gates
remain open.

The runtime and API package are the pinned compiler identities in
`fixtures/types/oracle-manifest.json`. The original probe scripts live in
`.cc/temp/type-presentation-{summary,deep,wide}-probe.mjs`; their inputs and
observed facts are retained here, independently of that scratch directory.
