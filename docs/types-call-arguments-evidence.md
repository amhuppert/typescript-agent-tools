# Contextual arguments and scalar relations

This bounded Ousterhout checkpoint uses one immutable accepted Project and
request-local native Evaluator. TypeScript 7.0.2 at
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b` supplies expected answers. It
establishes actual contextual scalar argument checking and native directional
relations for the admitted facts. It does not close full call, inference,
structured-relation, checking-prelude or resource gates.

## Complete questions and failure order

`evaluation/contextual.rs` owns `checkExpressionWithContextualType` and
`isLiteralOfContextualType`. Contextual literal regularization asks about the
literal category rather than equality with a particular literal value; the
later relation rejects a mismatched value. Object/context-sensitive expression
construction retains its actual unsupported dependency until its full context
and inference owner exists. This operation is distinct from the normal
expression-site cache and quick initializer question.

`evaluation/relations.rs` owns `isTypeRelatedTo`, `isSimpleTypeRelatedTo` and
the established union recursion. Assignable, Subtype, StrictSubtype and
Comparable use one operation returning `Result<bool, DemandError>`: ordinary
False rejects a candidate, while a blocked comparison remains a failure.
Comparable preserves its reverse-simple check; canonical identity is True for
StrictSubtype too. Source unions use Each for ordinary relations and Some for
Comparable; target unions use Some. Native undefined stripping and correspondence
precede the corresponding full target comparison. Constraint normalization,
structured relations, Maybe/cache history and compiler-limit owners remain
required. The current object-to-primitive early False is separate from the
unsupported structured-comparison path.

`isSignatureApplicable` checks actual this first, then each parameter's distinct
call-read type, its contextual argument and Assignable relation. The first
rejection stops before later arguments. Too many arguments still reject at
arity before argument checking. Chosen checked-call rejection consumes
`InvalidSemantic/overload_recovery` even when native recovery displays the
candidate's healthy-looking return. A missing argument name that is actually
visited consumes `InvalidSemantic/unresolved_symbol` at that name. Genuine Any
checks its supplied argument rather than bypassing the dependency.

## Final compiler and Project proof

`tools/type-native-call-arguments-oracle.test.mjs` passed three tests with zero
skips in `vrun-ecf6e762-84e8-4431-a59b-a0d721bfe7c3`:

- 62 fresh public demands pin 31 cross-file named aliases in strict and loose
  null profiles, including canonical exponent, bigint and escaped-string
  literals, Boolean, Symbol, optional/void/Any/Unknown and source unions.
- 24 same-checker observations pin before/after-summary schedules. Wrong this
  and an earlier wrong argument reject before a later Missing name; actually
  visited typed/untyped Missing names record their own 2304. Rejected literal,
  primitive, Never and union arguments record 2345. Unrelated global diagnostics
  are not a blanket root-invalidity predicate.
- 52 fresh directional scalar comparisons pin literal-to-primitive and reverse
  distinctions, unequal literals, Boolean's semantic union, Any/Unknown/Never,
  void/undefined direction, strict/loose nulls and primitive/literal unions.

`tests/type_call_arguments.rs` had genuine compiled five-test assertion RED
`vrun-3c2c3584-2032-49a0-8c48-071265f4ee82`. All five failed at the missing
contextual/this applicability operation. Production GREEN
`vrun-5bb6ed70-2c39-4e3f-b3fb-96dc8fe5b35b` matched that single file with zero
skips. It asserts final Found graphs, selected return types and exact native
invalidity reasons/locations across the rule's variants and both option states.
Intermediate `vrun-3be89884-56da-47ac-8bd0-4912d80a1339` was setup-only: an
unrelated shared flow constructor referenced a removed Pending variant. No
behavior tests ran there, and it is not a RED or regression claim.

The Annotation recipe boxing in the same stage was mechanical, with no new
behavior to pin; the compiled final gate covers the resulting wiring.

## Artifact identities and limits

Hashes below were read together after the final scoped GREEN; concurrent
owners' separate files and later edits are not included in this snapshot.

| Artifact | SHA-256 |
| --- | --- |
| functions.ts | `9f94307f599a57c46127b569b8d325794d7a8fd6344ba26da49ac9f036912240` |
| consumer.ts | `cc760efccf7c3d9d74fd3cd849ef4c6e876ea706569bcef4e78fdf3802954c7c` |
| relations.ts | `beae480171b3de080ef60c8925a2d380026a9eb18155ebbacea6a5cf91c1cb51` |
| tsconfig.json | `012bb1c5b3868407506d49b9ac47eb7321292f60e4e0b13b67263acced3c5577` |
| loose.json | `fe4cd0fa7a1f6af662adac9e390e37cccc238643c523210fd5208f7ce1fe84d7` |
| native oracle | `08f41193ab0f9a33a407c058844755d7b90855fc742b3aa1a7442a64a734469d` |
| final Project tests | `ce282fc585c1d8d0a8eea7265177f6b946e2f2340e878289a09fdf2bc485cc21` |
| relation owner | `9df68deba9857f45bad02b0013b2b57365902e9f645be17ef2716f0c7cb43ed3` |
| contextual owner | `7ce7a55afd49f74261d54a18813eb243c49f6a1f65e65304ac21889b53072c72` |
| call owner | `d8cd4d8b9031ffd1cef5e613309be9ca68145e28c9a8791f5f0a03ac03b7b08b` |

Generic/contextual inference, overload ordering and retries, receiver this,
rest/tuple/spread, New/Tag, composite structured/enum/unique-symbol relations,
native cache and Maybe semantics and every compiler boundary remain mandatory
ongoing implementation. The next complete consumer is the executable
outermost-to-innermost checking prelude, using actual call attachments and the
same native signature operation, without an added chosen-return demand.
