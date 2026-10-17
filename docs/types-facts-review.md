# Bounded native type-facts review

The Ousterhout review covers `evaluation/facts.rs`, its shared Model
constituent/filter/intrinsic factories, and the existing unary truthiness
consumer. It assumes one captured Project and one request-local Evaluator.
TypeScript 7.0.2 commit `2bd066d87f5bafd315be9f40889d0a60b9e58e0b`
checker.go `getTypeFactsWorker` (30739), `isFunctionObjectType` (30896),
`getTypeWithFacts` (30907), `getAdjustedTypeWithFacts` (30916) and
`recombineUnknownType` (30958) supply the decisions. This was read-only;
no new test or production change was needed for an unsubstantiated defect.

No concrete false answer was found in the admitted
primitive/literal/special/Boolean/ordinary structured family. Facts are
possibility masks, not runtime truth predicates. Native mask bits, nullable
strict/loose states, exact zero/empty payloads, branch ordering and object
early-out match the source. The same full fact owner now serves expression
truthiness; caller-specific typeof/condition adjustment remains a different
operation. A healthy facts mask for Any does not by itself establish an entire
typeof narrowing rule.

The object mask early-out precedes member acquisition. An actual anonymous
empty object differs from Interface/Reference and intrinsic nonprimitive
object; display name does not decide emptiness. Known call or construct
collections establish function-object facts. A bind property without the
actual global Function subtype comparison remains a located obligation;
its absence is the native quick negative case. Evolving-array and other
unadmitted flags remain future dependencies.

Parameter facts are safe for the present admitted family because the separate
constraint demand's `established_base_constraint` guard refuses parameter
chains and References before returning a purported base. The direct constraint
handle must not become a universal replacement for
`getBaseConstraintOfType`: native chains, intersections, circular/recursive
base resolution and mapped/indexed forms need their complete owner. A missing
constraint gives the native Unknown facts; an actual failed constraint demand
stays failed, including under narrow masks.

Model's constituent operation retains Boolean's semantic false/true union
despite its primitive presentation. Filtering preserves the original identity
when every constituent survives, preserves every Never identity without a
predicate call, and uses the native sorted-list union factory for subsets.
The Unknown replacement is the actual cached intrinsic
Undefined/Null/emptyObject union, and recombination checks that identity rather
than equal text. Denormalized union origins remain outside the current admitted
factory's contract and must be retained when that family becomes reachable.

Nullable/intersection/global NonNullable adjustment remains explicitly pending
when consumed. It never substitutes the unchanged healthy type. The parent
reader must own its complete condition/flow use and preserve native scheduling
and error provenance. Reported final two-test GREEN
`vrun-717427dc-632c-41d0-a808-44f4314644e6` proves the actual unary
truthiness consumer after its behavior RED. The still-unused private
filter/adjusted operations and the captured compiler observations do not prove
their future public read consumer or close the full flow/relation gate.
