# Adversarial review: null, void and syntactic flags

Read-only Ousterhout review of the implemented `Source.variable_type`, expression
acquisition/checking, initializer widening, shared syntax flag owner and native
special-expression controls. This review found no blocking contradiction in the
admitted cases. It is a bounded source comparison, not a full native type/flow or
resource verdict.

Pinned source commit `2bd066d87f5bafd315be9f40889d0a60b9e58e0b`:
`ast/utilities.go` 971–1012 (`ModifierToFlag`), 1154–1185
(`GetRootDeclaration`/`getCombinedFlags`); `checker/checker.go` 16565–16623
(`getTypeForVariableLikeDeclaration`), 17580 (`isNullOrUndefined`), 10814
(`checkVoidExpression`), 18155 (`widenTypeForVariableLikeDeclaration`).

The full sixteen-token modifier mapping and root/list/statement flag traversal
match the pinned owner. Syntactic modifiers, node storage flags and effective
semantic flags remain separate questions. The implementation routes existing
`has_modifier` and binding node-flag questions through the common owner; this
avoids a local export-bit fix that would leave a second traversal policy.

The automatic local-variable condition preserves annotation-first ordering,
noImplicitAny, export/ambient exclusions, mutable storage flags and parenthesized
null classification. Resolved intrinsic undefined and automatic arrays remain
explicit pending operations. AutomaticAny has its own internal identity and
cannot be admitted as an ordinary healthy any flow read from its public spelling.

`checkVoidExpression` only schedules deferred checking and returns the shared
undefined widening type. The fresh-demand operation therefore correctly avoids
consuming MissingValue in a void operand. Whole-program diagnostics observe a
different history and cannot be turned into an unconditional root dependency.
The fixed selected-site enclosing-call prelude remains independent: selecting a
site actually inside a call must still run that prelude first.

Applying complete widened_type after initializer literal handling is required,
including const nullable initializers in the non-strict profile. The new private
widening null/undefined identities preserve this question without conflating it
with literal freshness. Unary nullable diagnostics do not themselves invalidate
an otherwise established result when the native return consumes no recovery.

Registered native special4 GREEN `vrun-4a377cee-1ad7-4d92-8276-3a52ccd5c8a7`
was reported by root, alongside fresh four-profile compiler controls. This
review did not independently repeat that successful file. The inspected tests
cover local/exported var/let/const, absent/null/void initializers, parenthesized
null, nullable unary questions and strictNullChecks × noImplicitAny. Ambient
local-null and nested/destructuring shared flag final outcomes were additional
proof refinements sent to root; source parity alone does not claim those cases
executed. Broader flow, automatic arrays, intrinsic-undefined resolution,
unique-symbol widening and full resource gates remain open.
