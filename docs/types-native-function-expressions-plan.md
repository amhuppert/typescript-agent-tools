# Native function-expression checking

The operating envelope is an immutable accepted Project with one request-owned
evaluator. The native owner is `checkFunctionExpressionOrObjectLiteralMethod`
and its contextual worker (checker.go 10090–10180), with shared
`getTypeOfSymbol`, `getSignaturesOfSymbol`, parameter/return caches and the
existing contextual and inference sessions. A function expression is not a
second anonymous structural type assembled from its printed signature.

The existing final before-use syntax test is genuine RED at the ordinary IIFE
callee. Its compiler baseline covers ordinary, async and generator IIFEs,
deferred arrows and functions, and annotation-sensitive accesses. Generator
inference remains excluded; syntax queries do not infer a generator body merely
because a generator is an ancestor.

`bindFunctionExpression` must retain one actual Function symbol for named,
nameless and arrow expressions. Named local lookup and anonymous declaration
association share that identity. The actual binder ContainsThis flag supplies
`HasContextSensitiveParameters`; the context-sensitivity owner performs return
work before yield work and uses native `IsPartOfTypeNode` traversal.

The expression owner schedules the skip-context-sensitive/context-free branch,
contextual-signature acquisition, re-entry check, ContextChecked installation,
canonical call-signature acquisition, annotated inference, parameter assignment,
optional eager return-body resolution and signature checks in native order.
Failures after ContextChecked remain retained failures, not successful checked
sentinels. Context-free types retain their actual function symbol and distinct
non-inferrable signature/type flags.

Canonical signatures need shared contextual type-parameter and this-parameter
slots. Refreshing inventories would leave active relation frames with stale
snapshots, so the signature owner provides shared one-time installations and
all consumers read the same slots. Parameter raw types use their canonical
symbol caches; the lazy MemberValue wrapper must not install a second answer
when native intentionally leaves context-sensitive parameter types unresolved.
Return caches already belong to the actual signature identity.

Proof obligations include final ordinary/named/generic/arrow expressions,
contextual parameter/this/return types, initialized and rest parameters,
contextual re-entry during overload resolution, inherited contextual generics,
skipped context-sensitive argument passes and cached original failures. Existing
before-use and flow suites remain compatibility gates. Native mutation-join
identity evidence is separate and is recorded in types-basic-flow-review.md.

The current producer checkpoint has a pinned compiler baseline:
`vrun-1f9acfb5-57cf-4ff3-aa22-a22b3a022a64` (36 ordinary, named, arrow,
generic, initialized, rest, IIFE, contextual-parameter and contextual-this
questions over strict, loose and exact profiles; no skips), including actual
unparenthesized parameter queries, literal-Boolean return, and the Boolean
equality arrow whose predicate path acquires its parameter before unchanged flow. The final consumer
is `tests/type_function_expressions.rs`, with expanded and unexpanded queries.
The first baseline receipt `vrun-b0cabe70-832b-4f45-903f-052e55a26786` exposed a
fixture error: Array was module-local and native rest display recovered to `{}`.
The maintained fixture now declares actual globals in `globals.d.ts`.

`check_signature_declaration` now resides in the shared type-check child; both
function expressions and function/constructor type syntax use it. It retains
constraint/default checking and the subsequent all-child default-reference
walk, raw parameter cache reads, primary versus secondary declarations,
initializer checks, bodyless initializer short circuit, rest assignability,
and return annotation checks. No function body is checked merely to establish
a function value. Binding-pattern parameter factories, contextual object
ThisType and async/generator return factories remain explicit dependencies.

Adversarial review found and corrected contextual this installation order:
the new unresolved parameter clone is installed before reading context.this,
so recursive work observes the actual shared signature field. The clone owns
a distinct raw demand and copied declaration, not the source's raw demand.
The existing resolved-type guard and contextual no-cache rule apply to that
question, including explicit this parameters. Type wrappers do not introduce
another cache for canonical member values.

The adjacent automatic-array change retains createArrayType(autoType) as a
separate marker, includes it in automatic flow setup, excludes it from constant
closure capture, and converts it to anyArrayType only at native convertAutoToAny
sites. The initializer gate admits direct `[]`, including const declarations;
parenthesized `([])` is a different native syntax case. Invalid global-array
factory recovery remains invalid. Intrinsic undefined symbol retention is covered separately in
types-native-intrinsic-undefined-plan.md; no spelling-based identity is used.
