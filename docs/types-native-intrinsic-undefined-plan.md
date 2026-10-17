# Native intrinsic undefined identity

The operating envelope is one immutable Project with request-local evaluation.
Native creates an actual Property symbol with no declarations, installs it in
Globals only if no entry of any meaning exists, and preinstalls its
undefinedWideningType. Local symbols and raw aliases retain distinct identity.

The retained Store now owns the actual declarationless symbol and a typed
producer tag. Its existing global table, lookup, aliases, canonical identity,
reference matching and loop keys remain the shared mechanisms. An alternative
Intrinsic arm in every ReadOrigin/BoundValue/cache-key consumer would duplicate
identity policy and force unrelated callers to reconstruct symbol behavior.
The retained header avoids that cost without borrowing an existing declaration
or inferring identity from the text of a use.

`addUndefinedToGlobalsOrErrorOnRedeclaration` is applied after global
augmentation merging and before ambient modules or nonglobal augmentations,
matching initializeChecker's actual lookup schedule. A global type-only entry
suppresses insertion and leaves a VALUE lookup unresolved; a global value entry
keeps its real value even when native emits a redeclaration diagnostic. Lexical
navigation still reports the intrinsic as a built-in non-reference target.

`getTypeOfSymbol` observes the producer tag and returns the Model's existing
undefinedWideningType. `checkIdentifier` uses its existing non-variable path,
including an assignment error before the early return. Effects observe its
actual absence of a declaration annotation. `isNullOrUndefined` owns parentheses,
Null syntax, and exact raw Identifier symbol comparison inside variable recipe
acquisition; aliases and shadows never qualify through spelling.

Native controls cover strict/loose/exact direct reads, const/automatic inference,
parenthesized initializers, typeof, local parameter shadowing and its mutable
initializer, local type-only shadowing, exported declaration aliases, invalid
exported type-only values, intrinsic assignment, and global type/value entries.
The first capture-only receipt is 780af4b2-876a-4a50-9cb7-df5a8e9848ff; it is
observation evidence, not an assertion baseline. The maintained harness asserts
summary and native errorType for every question: 38 assertions passed in
`vrun-6e9ac06e-9cec-4454-ab1f-6783d27c67e1`, with no skips. Final tests are
`tests/type_intrinsic_undefined.rs`, with expanded/unexpanded results and the
existing FlowRequired boundary for strict automatic inference.

Next coordinated validation must run the final suite and a counterfactual that
disables only the actual global intrinsic installation, followed by restoration.
Navigation/reference compatibility must remain green. No Rust validation has
run for this stage yet.
