# Generic call ownership review

This bounded Ousterhout review assumes one immutable accepted Project, exact
captured sites, canonical request-local TypeIds/SignatureIds and the existing
native mapper, resolution and contextual operations. It reviews
`types-native-call-inference-plan.md` against pinned checker.go:9001, 9198,
19198–19243 and 21859–21914. It is a plan review, not a final inference or
generic-call implementation proof.

The chosen complete evaluator operations hide the necessary canonical-slot,
ordered-default and cache history. A spelling map would lose fresh local
parameter identities and independent read/display/call questions. Keeping
arbitrary member signature instantiation separate from the native
getSignatureInstantiation cache matches the source and avoids invented
identity reuse. Ordinary constraint rejection remains data for overload
selection; consumed missing/default recovery retains its original failure.

One scheduling detail must be explicit in implementation and final controls:
checkTypeArguments fills the entire argument vector first, but its constraint
loop ranges only over actual supplied type-argument nodes. It does not check
every filled default against every parameter constraint. getMinTypeArgumentCount
asks syntactic default presence rather than resolving the default type. A
broken omitted constraint must therefore remain skipped when that native
question is not visited. Conversely all needed defaults are acquired during
fillMissingTypeArguments before any supplied-argument constraint can reject a
candidate. Forward defaults consume the actual errorType placeholder and do
not become an ordinary False relation or an unconstrained Unknown fallback.

The current narrowing owner has a Normal reference consumer and a complete
exact contextual-node push/pop. Its two relevant mode facts are not the full
CheckMode algebra. Before Inferential/context-sensitive consumers are admitted,
the existing contextual/checkExpression/cache operations must share one native
mode representation. Inferential skips the union-constraint predicate entirely;
Contextual alone is already different from cacheable Normal, while assignment
getInitialOrAssignedType deliberately asks narrowing in Normal mode. A second
call-private context stack or treating all non-Inferential checks as cacheable
Normal would change the source schedule. Exact context-node identity and
restoration after failed nested checks must remain in the complete operation.

The plan preserves actual resolving/recovery/healthy cache properties and
temporary loop-resolution restoration. No extra defect was found in that
ownership choice. Full inference candidate priority/fixation, return-context
cloning, relation Maybe/cache behavior, default circular sentinels and error
candidate reentry still need their own source-backed final Project controls;
the scalar overload and explicit-argument increments do not establish them.
