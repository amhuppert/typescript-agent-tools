# Native explicit generic calls

This increment extends the existing native Calls owner with explicit type
arguments. It is part of the full selected T/Q3/Q4 implementation; inferred
arguments, contextual callback retries, generic rest/spread arity, complete
overload recovery, relation limits and public delivery gates remain required.
The source is the pinned TypeScript 7.0.2 checker at commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`.

The ownership choice follows the two designs in the historical call-inference
plan (archived in Git history). A map keyed by parameter spelling would lose
canonical cloned parameters and independent parameter/return questions.
The implementation uses the existing canonical mapper, constraint/default and
signature factories, with one native call-instantiation cache keyed by actual
SignatureId and the ordered filled TypeId vector. Arbitrary member signature
instantiation remains a different question and does not acquire this cache.

`checkTypeArguments` in checker.go:9198 fills the entire vector before any
supplied-argument constraint check, then checks only actual source argument
nodes. Each constraint is acquired through the shared guarded/raw constraint
owner, instantiated against the complete vector, and passed through
`getTypeWithThisArgument` before Assignable comparison. Ordinary False rejects
that candidate and permits a later overload. A consumed missing/default/error
failure preserves its original location rather than becoming False.

`getSignatureInstantiation` and its cache in checker.go:19198–19243 fill again,
apply each actual parameter's own mapper, and invoke `instantiateSignatureEx`
with erasure. Erasure suppresses fresh local parameters while retaining distinct
graph/read/display/call and return questions on the new actual signature.
Only a completed factory result is installed in the native instantiation cache.
The same shared fill owner also replaces the TypeReference caller's prior
inline copy. Fully supplied arguments do not demand defaults. Missing scalar
forward-default slots consume the actual errorType placeholder even when the
compiler emits no diagnostic.

The registered compiler baseline is
`tools/type-native-generic-calls-oracle.test.mjs`, using
`fixtures/types/native-generic-calls`. GREEN
`vrun-8828dea6-0eb5-421a-a229-14d24e0a1553` executed both tests with zero skips:
52 fresh type controls and 36 same-checker beforeSummary observations across
strict and loose null profiles. Earlier GREEN
`vrun-885f4613-972d-4820-a7e2-73e747adfc14` covered the prior 50/32 matrix.
The post-terminal input capture in
`.cc/temp/native-generic-calls-inputs.json` names its hash timing explicitly;
it is not an atomic capture of source and compiler execution.

The final Project behavior file is `tests/type_generic_calls.rs`. Its genuine
compiled RED is `vrun-1c39add4-ae93-429d-9d14-6258ee09c1fb`: five tests executed,
all failed at the pending generic-call operation, with no setup failure or
skip. A missing source type argument already reached its original invalidity
before the next missing argument stopped at that pending operation. The
final registered GREEN is `vrun-02c482de-8ebc-4434-b45e-b378dd2cdcab`:
one matched file, all five tests passed, zero skips. Earlier group attempts
that failed compilation are setup failures and are not behavior-level REDs.

The final assertions distinguish exact intrinsic/literal/union roots, dependent
defaults and successful later candidates from fully rejected overloads and
actually visited failures. Two paired controls pin the schedule: omitted
`U extends MissingConstraint = number` stays healthy number, while needed
`U = MissingDefault` is acquired before a supplied earlier number violates a
string constraint. The latter records 2304 before 2344 in the same-checker
phase observation. Strict and loose variants assert final public outcomes and
the original definition or argument location.

This evidence does not establish the full inference/contextual/recovery
operation. Native fill mappers can retain the mutable result vector while lazy
default types escape; the current copied mapper targets have been checked for
the admitted eager scalar/reference substitution, while TypeLiteral outer
mapping remains explicit pending. Lazy default families require their exact
mapper-history proof before admission. These controls also do not establish
error-candidate reentry, generic rest arity, new/tagged/optional calls, structured
relation behavior, or a complete resource gate.
