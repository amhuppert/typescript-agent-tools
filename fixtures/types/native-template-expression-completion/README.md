# Template-expression continuation

These cold TypeScript 7.0.2 queries compare the compiler hook with the public API
in strict, loose and noCheck profiles. The hook observes already-returned
expression/type results; it does not acquire a type to inspect it.

`substitutions` is the smallest recovery witness. Each generic value's lazy
constraint is missing; native checks both substitutions and returns String.
The product currently admits the first constraint completion in
`maybeTypeOfKindConsideringBaseConstraint`, before checking the second value.
`constantContext` and `elementContext` retain the same substitution order under
the two syntax context gates. `contextual` acquires the contextual parameter's
constraint before the later argument's constraint. `contextualConditional`
acquires both conditional branches through the contextual const-type predicate,
then the later argument. Every selected native type is non-error.

Healthy controls cover runtime widening, evaluated constant freshness, a
contextual template argument and symbol conversion. Symbol conversion emits
TS2731 but returns ordinary String without an erroneous returned-type event.
This distinguishes an emitted diagnostic from consumed errorType recovery.

The fixture proves the observed substitution/context paths, not every possible
alias/enum constant evaluator or tagged-template path. Error event counts do not
specify the product's cache-replayed cause count. The public test must check
ordered source locations rather than treating these event counts as a golden.
