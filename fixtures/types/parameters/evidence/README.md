# Native parameter controls

These bounded controls preserve the actual TypeScript 7.0.2 questions used by
the first canonical parameter increment. They do not prove complete generic
checking, circular/base constraint guards, inferred/mapped parameters, class
targets, instantiated defaults or member mapping.

`type-parameter-native-control.mjs` and its fifteen-row JSON result use fifteen
fresh public API sessions. Each gathers config, Program, syntax and bind
diagnostics before the semantic question; every such inventory is empty. The
root question is `GetTypeAtLocation` on the containing TypeReference node,
following the product's type-reference position selection. Asking that native
operation on its bare Identifier is a different question. Root identity equals
the parameter declaration's canonical `GetDeclaredTypeOfSymbol` identity.
Ordinary `any` and `unknown` constraints both produce genuine `unknown` through
`GetConstraintOfTypeParameter`. The missing constraint produces a true native
error type. The default-node rows are explicitly node questions, not the
default-resolution operation.

`default/` preserves the independent same-checker Go control supplied by the
oracle owner. Three fresh frozen-input replay checkers call the actual native
`GetDefaultFromTypeParameter`: basic has no default, a later merged declaration
supplies the literal `"ready"`, and `MissingDefault` returns native unresolved
recovery. The last request adds TS2304 after that demand. These replay snapshots
also preserve their eager noLib TS2318 inventory before and after the question;
that inventory is distinct from the default-demand delta and from the public
API's empty pre-demand input inventories. No full semantic pass is requested.
The answer includes hashes of the exact strings replayed by the Go checker.

`manifest.json` freezes these nineteen artifact files after observation. Public
API input files were stable generated research inputs; their recorded hashes
are a post-run inspection, not an atomic capture of native reads. Existing
merged JSON fixture inputs and the pinned compiler/API identities are separately
frozen by `fixtures/types/json-generic-targets/evidence`. The copied scripts keep
their original relative imports: replay them from their documented `.cc/temp`
paths, rather than rewriting archived source bytes.

Final product TDD is in `tests/type_generics.rs`. Its first registered run,
`vrun-97bd635e-b9dc-4458-9fa0-6a23bf85bfa7`, compiled and executed three tests;
all failed at the expected missing canonical parameter declared-family
operation. That receipt proves a behavior RED and is not a success claim.
