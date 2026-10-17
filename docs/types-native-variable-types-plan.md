# Variable and destructuring type construction

This stage moves `getTypeForVariableLikeDeclaration` out of the existing three
Source/evaluator copies. Source retains actual declaration and expression
nodes; one evaluator operation owns for-in/of, binding-element, annotation,
catch, automatic, parameter-context, initializer, property-flow, and implied
pattern precedence. The operation returns native nil separately from a failed
question. Symbol reads then call the separate
`widenTypeForVariableLikeDeclaration` operation; `getTypeOfNode` uses the raw
optional result. Existing symbol resolution caches remain the only owners of
resolved variable/parameter/property types.

Keeping initializer shortcuts in each consumer was considered and rejected:
binding patterns depend on their parent's optionality-free read, initializer
context, iteration, indexed-access node, and synthetic flow reference. These
are observable native questions. A generic syntax-tree visitor would also
hide their different demand order. The chosen Source recipes retain the facts
needed by these exact native operations, without doing any child type work.

`getTypeOfAssignmentPattern` is an independent checker entry. It acquires the
right-hand value/iteration result and calls the complete checked destructuring
assignment owner. It does not return an initializer's type directly. Nested
object and array patterns recurse through that same entry and then ask the
specific checked property/element operation. Assignment checking preserves
reference validity, accessibility, defaults, rest position, and reporting
relation consumption even where native diagnostics leave the source type as
the result.

Binding patterns use actual captured node identity for the native contextual
pattern stack and PatternForType associations. Implied object members reuse
the property arena's known-value factory; implied tuples reuse the tuple
factory and an actual fresh reference clone when a pattern is attached.
Object rest uses the shared spread-symbol operation and actual global Omit
alias for generic sources. IndexInfo is the existing immutable shared handle.
No independently inferred property identity or duplicate member table is
introduced.

Existing operations are reused only for the same native question: checked
iteration, literal property keys, indexed access, tuple slices, optionality,
facts, contextual parameter types, cached expression checking, reporting
relations, and the created-reference flow interpreter. Destructuring's
synthetic element reference has a fresh node, the source parent's FlowNode,
and a literal property name; a source span is not its identity. F still rejects
a changed final flow result after the real computation and dependency reads.

Required evidence includes object/array/nested patterns, defaults with and
without a root annotation, omitted elements, ordinary/rest elements, parent
Any/nullable/union/generic types, property/index absence, catch options,
for-in/of, contextual and untyped parameters, parameter initializer padding,
and original failures acquired before a later diagnostic recovery. Tests must
compare final queries to the pinned fresh-checker baseline, with separate
declared/flow identity observations for F. Existing destructured-assignment
tests concern `getInitialOrAssignedType` and do not prove this new checked
assignment question.

This source stage is coordinated with the new `getTypeOfNode` entry. Real
missing producers (generic Omit/mapped types, class-property flow, unique keys,
or checked assignment elaboration) retain precise located dependencies until
their owner is implemented. They are required work, not an admitted successful
substitute or a reduced delivery contract.

The first native declaration-query matrix covers 17 markers in strict, loose,
exact-optional and unchecked-index configurations: 68 final summaries and
non-errorType observations passed in `vrun-331f81d3-d18d-4bf0-a665-3c99bf92a2f3`.
`tests/type_variable_patterns.rs` consumes the same expectations; it is staged
for the coordinated Rust checkpoint, not yet a product verdict. Bounded peer
reviews checked variable precedence, cache and default ordering, synthetic
flow references and assignment elaboration. The review's `noCheck` finding
is corrected through the existing configuration reader; a discriminating
native/final elaboration control remains required.

The first compiled final matrix (`vrun-04ef3b28`, stage11) passed one group and
stopped three groups at BindingElement selection, before entering this producer.
The selection owner now treats the actual binding declaration identifier as a
variable declaration and routes it through the existing canonical symbol/value
operation. Expectations are unchanged; the repaired final matrix awaits the
coordinated checkpoint.
