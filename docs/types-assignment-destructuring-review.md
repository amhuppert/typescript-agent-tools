# Destructured assignment dependency review

This read-only Ousterhout review compares the current plan with TypeScript
7.0.2 `flow.go` 2209–2368. Its envelope is the immutable loaded Project and
one request-local evaluator; it makes no production or broad flow gate claim.

No blocking ownership problem was found. Source supplies the actual syntax
parent questions, while the existing initial/assigned-type operation executes
their dependency order. That boundary avoids repeating parent/default rules
in every flow consumer. An iterative continuation stack preserves native
parent recursion without introducing a new assignment binder.

The important ordering requirements in the plan match the source. Binding
elements acquire their parent's initial type before property, array-element or
spread projection. Assignment-array elements recurse through the actual
containing assignment; a destructuring default obtains that source projection
before its default expression. Property projection acquires the assigned
container before checking its key expression, then asks the ordinary property
question before applicable index lookup. Array projection first asks the whole
tuple-like predicate/projection, then iteration and the no-unchecked-index
policy; spread performs iteration before the real Array factory.

The documented `any` controls correctly keep three different native results:
ordinary array iteration supplies Any, spread constructs an actual Array from
its element, and a missing `any` object property consumes ErrorType. They must
remain facts from those owners, rather than a shared Any shortcut. A missing
projection, an invalid computed key or a consumed iterator/global-array
recovery cannot be hidden by later assignment reduction retaining the declared
type. The source does not apply the reference's final non-narrowable gate to
intermediate assigned-type questions.

The existing apparent-type/structured-signature operation must remain the
shared evaluator owner when property or index applicability reaches those
dependencies. The new `getLiteralTypeFromPropertyName` operation is correctly
separate from the binder's declaration-key question. Full tuple/iterator,
default, rest, nested-pattern and option-dependent final controls remain
required. The review did not execute product tests or establish a resource
bound for the planned continuation stack.

The staged Source `property_names.rs` and global readonly factory were also
read against checker.go 26640–26691 and initialization 1365–1367. Numeric names
retain their actual checking site; computed names preserve the direct native
forbidden-`in` test without stripping parentheses. Global readonly identity
inspection and construction share one sentinel-fallback operation. Binding
failures remain failures; only native recovery selects the actual Array
fallback, and only construction consumes its final recovery cause. No blocking
defect was established for the current destructuring consumers.

One future completeness gap is explicit: the Source branch for
`JsxNamespacedName` expects a direct scalar `text` field, while the accepted
parser emits only `namespace` and `name` children (parser/jsx.rs 280). Native
`GetPropertyNameForPropertyNameNode` obtains that name's composed Text. Current
destructuring cannot produce this family, so this is not a measured admitted
false answer. The future JSX consumer must use the exact composed-text owner
or retain a precise unsupported dependency; this branch does not prove full
checker property-name coverage.

The coherent evaluation helpers were subsequently read without source edits.
Computed-name evaluation installs the actual Normal expression-cache result
before diagnostic dependencies; a nullable diagnostic branch skips the native
relation checks, and diagnostic-only results do not replace a healthy type.
Own structured-property absence stays distinct from the unsupported global
augmentation question. Tuple-like acquisition tests property existence before
reading its value, then array-like acquisition consumes the actual readonly
Array relation before the length read. Iteration keeps the native early Any
identity and exact ordinary Never nil-result-to-Any conversion; unimplemented
iterator dependencies remain explicit failures. No-unchecked-index processing
uses the actual Missing identity independently of exact-optional mode. No
admitted false answer was found in this bounded source review; final product
controls and full iterator/tuple/resource closure remain separate obligations.
