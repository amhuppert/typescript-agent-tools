# Declaration initializer checks

Pinned native owner: TypeScript 7.0.2 `checkVariableLikeDeclaration`, checker.go
5769–5937; parameter continuation at `checkParameter` 2642. This is the local
query checker profile, not a whole-program diagnostic command.

The stage25 contextual-pattern Project gate failed at `initializedContext` with
“native reporting assignment relation for a declaration initializer is pending”.
Native contextual170 capture and maintained comparison passed. The selected
callback-body identifier forces actual contextual parameter assignment and its
source-check operation; its default `{ a: 1 }` is checked against `{ a: 0 }`.
The existing final test and native expectations remain unchanged.

The shared variable-like source-check owner now sequences the actual annotation,
computed name, binding-element accessibility prelude, nested binding elements,
bodyless-parameter exit, and pattern initializer checks. Patterns exit before the
canonical-symbol branch. Empty patterns retain native non-null or iteration
checks. Primary and secondary declarations use the reporting relation's actual
error node separately from the initializer expression; native diagnostic-only
False does not replace the inferred type. Secondary declarations preserve both
error-type guards and the Identity-before-Assignment gate.

Widening has one raw-input owner and one declaration-type entry. Its reportErrors
flag is independent of includeOptionality. Using checks pass the acquired
initializer value into that raw-input owner and retain checked Disposable global
identity gates. Missing disposable globals skip that relation as native does;
there is no consumed fallback type at this identity-only branch. Rest-parameter
validation independently reacquires the raw parameter symbol after the variable
check, and excludes binding-pattern names.

The syntax owner ports complete NodeCanBeDecorated eligibility, including legacy
private names, standard versus legacy class forms, body presence, and explicit
this-parameter exclusion. A reached decorator operation remains an explicit
existing prerequisite before child acquisitions. Private/protected class access,
CommonJS alias validation, and merged-export alias declaration-space traversal
remain separate required owners; they are not claimed by this initializer
reporting repair. Public/super/abstract accessibility predicates are shared with
the ordinary property-access operation rather than copied into binding checks.

Rejected design: handling only `initializedContext` at its call site or replacing
failed assignability with a successful no-op. Both would miss nested-element,
empty-pattern, secondary-declaration and diagnostic-display acquisitions. The
existing relation operation owns reporting and elaboration; declaration source
checking owns when it is called.

Stage26 contextual170 final regression now passes all four groups:
`vrun-bba34629-df0d-486a-a871-8a79c7dee232`. Mapped final checks also pass
(`vrun-598e93e5`). Dedicated declaration-check branch evidence and broader
source-check/recovery gates remain required before claiming the whole reporting
owner is fully proven. Native filter-origin evidence is an independent
proof for the shared TypeFilter controller, tracked by its own fixture/harness.

The dedicated native declaration matrix now contains 25 roots per strict,
loose and exact profile. Bootstrap `vrun-bc88e321-4b50-4f3f-aa56-91a35bf83529`
completed all 75 public-API/hook comparisons; each profile failed only its absent
expected file. Installed baselines prove 24 genuine worker-entry roots and one
quick-call control. Duplicate type-literal declarations reach actual secondary
ValueDeclaration checks and native 2717 diagnostics. Bodyless scalar defaults
are unconsumed, but strict/exact pattern child checks demand the initializer
before the parent bodyless exit; loose omits that earlier prerequisite.

`tests/type_declaration_initializers.rs` is staged with exact healthy summaries,
expanded queries, primary/secondary display dependency locations and ordered
distinct source acquisitions. The strict null empty-object case preserves
actual ErrorType at the parameter, not the healthy outer arrow type. The
maintained comparison passed all 75 declaration queries together with the 54
filter queries: `vrun-1edb0833-36cb-4381-8813-9bc4822e4da8` (6/6 groups,
zero skips). The final Rust test is not yet claimed passing.
