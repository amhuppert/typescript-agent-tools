# Diagnostic cache recovery accounting

The recovered cold/warm controls require different final cause counts. The warm
query consumes two failed-relation cache entries as well as four diagnostic
displays. Its 30 missing-type events are required by the existing completion
contract; they are not duplicate locations to remove.

The fixture is [native-diagnostic-cache/cases.ts](../fixtures/types/native-diagnostic-cache/cases.ts).
`take(recovered)` is the cold query. The warm query checks
`observe((take(recovered), take(recovered)))` in one evaluator. The source type
contains a lazy `hidden?: MissingCache` property, whereas the target requires an
absent `required` property. The relation rejects before reading `hidden`; its
diagnostic display is the first consumer of the missing annotation.

## Measured native acquisitions

[The acquisition harness](../tools/type-native-diagnostic-acquisitions-oracle.test.mjs)
passed all three profiles, with zero skips, in
`vrun-5cdb4ddc-9680-469a-84a1-8f90848d16e5`. The receipt is
`.cc/temp/stage26-diagnostic-acquisitions-receipt.json`; raw rows are
`.cc/temp/native-diagnostic-acquisitions-{tsconfig,loose,untruncated}.json`.
A rerun of the harness now writes the raw rows to
`target/harness/native-diagnostic-acquisitions-{tsconfig,loose,untruncated}.json`.
The hooked final summary/error-type result matches the ordinary native API.
These are natural repeated calls in one checker, not repeated Project requests.

The observer records entry/exit and already-produced results of
`getTypeOfSymbol` and `getTypeFromTypeNode`, plus existing node-builder cache
events. It performs no extra semantic question. All operation stacks balance.
Diagnostic code arrays are source sorted and are not evidence of acquisition
order.

Every profile has the following observations:

| Observation | Cold | Warm |
| --- | ---: | ---: |
| Error-valued `getTypeOfSymbol(hidden)` exits | 2 | 2 |
| Error-valued `getTypeFromTypeNode(MissingCache)` exits | 4 | 4 |
| Source object display visits | 2 | 4 |
| Source object display cache hits | 1 | 3 |
| Source object display cache installations | 1 | 1 |

There are five independent failure-bearing acquisitions in the first source
transform, not six. Its first `getTypeOfSymbol` contains one of the four
type-node calls; the surrounding operation retains that child's completion once.
The remaining operations run in this order:

1. `addPropertyToElementList` reads `getNonMissingTypeOfSymbol(hidden)`. Its
   initial raw symbol read acquires the missing type node.
2. `requiresAddingImplicitUndefined` reads the same raw property again, before
   checking the optional/reverse-mapped guards. This reads its installed slot.
3. `serializeTypeForDeclaration` realizes the direct pseudo type after
   equivalence. Equivalence itself short-circuits on the actual error type.
4. `tryGetResolvedSymbolFromTypeNode` reads the annotation to ensure its symbol
   slot is resolved.
5. `tryVisitTypeReference` asks the annotation type before its reuse check.

The pinned source is TypeScript 7.0.2, commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`:
[property preparation](https://github.com/microsoft/typescript-go/blob/2bd066d87f5bafd315be9f40889d0a60b9e58e0b/internal/checker/nodebuilderimpl.go#L2459),
[implicit undefined](https://github.com/microsoft/typescript-go/blob/2bd066d87f5bafd315be9f40889d0a60b9e58e0b/internal/checker/emitresolver.go#L581),
[declaration realization](https://github.com/microsoft/typescript-go/blob/2bd066d87f5bafd315be9f40889d0a60b9e58e0b/internal/checker/nodebuilderimpl.go#L2224),
[error-type equivalence](https://github.com/microsoft/typescript-go/blob/2bd066d87f5bafd315be9f40889d0a60b9e58e0b/internal/checker/pseudotypenodebuilder.go#L362),
[resolved-symbol prelude](https://github.com/microsoft/typescript-go/blob/2bd066d87f5bafd315be9f40889d0a60b9e58e0b/internal/checker/nodebuilderimpl.go#L499),
and [reference reuse](https://github.com/microsoft/typescript-go/blob/2bd066d87f5bafd315be9f40889d0a60b9e58e0b/internal/checker/nodecopy.go#L416).

The nested lazy object and its enclosing source display each install the
transform's completion in their own serialized-node slot. The outer result
contains the nested result once. Later source-node cache hits skip those native
getter calls and return the installed completion under the product's
cache evidence contract (archived in Git history).
They do not add six fresh getter events or traverse the nested cache again.
Native publication and hit behavior are owned by
[`visitAndTransformType`](https://github.com/microsoft/typescript-go/blob/2bd066d87f5bafd315be9f40889d0a60b9e58e0b/internal/checker/nodebuilderimpl.go#L3089).

## The additional failed-relation cache reads

The two diagnostic displays in each failed call have different cache ownership:

- `reportUnmatchedProperty` runs inside `structuredTypeRelatedTo`. Its five
  failures are included in the Failed comparison installed by
  `recursiveTypeRelatedTo`.
- `reportErrorResults` runs after `recursiveTypeRelatedTo` returns. Its
  `reportRelationError` display contributes another five failures to the current
  invocation, but those five are outside the installed comparison.

This follows native
[`reportUnmatchedProperty`](https://github.com/microsoft/typescript-go/blob/2bd066d87f5bafd315be9f40889d0a60b9e58e0b/internal/checker/relater.go#L4338),
[`recursiveTypeRelatedTo`](https://github.com/microsoft/typescript-go/blob/2bd066d87f5bafd315be9f40889d0a60b9e58e0b/internal/checker/relater.go#L3061),
[`isRelatedToEx`](https://github.com/microsoft/typescript-go/blob/2bd066d87f5bafd315be9f40889d0a60b9e58e0b/internal/checker/relater.go#L2709),
and [`reportRelationError`](https://github.com/microsoft/typescript-go/blob/2bd066d87f5bafd315be9f40889d0a60b9e58e0b/internal/checker/relater.go#L4744).

Each call asks applicability twice: once during
[`chooseOverload`](https://github.com/microsoft/typescript-go/blob/2bd066d87f5bafd315be9f40889d0a60b9e58e0b/internal/checker/checker.go#L9001)
and once during
[`reportCallResolutionErrors`](https://github.com/microsoft/typescript-go/blob/2bd066d87f5bafd315be9f40889d0a60b9e58e0b/internal/checker/checker.go#L9625).
Both applicability invocations call
[`checkTypeRelatedToAndOptionallyElaborate`](https://github.com/microsoft/typescript-go/blob/2bd066d87f5bafd315be9f40889d0a60b9e58e0b/internal/checker/relater.go#L436),
which first asks
[`isTypeRelatedTo`](https://github.com/microsoft/typescript-go/blob/2bd066d87f5bafd315be9f40889d0a60b9e58e0b/internal/checker/relater.go#L170).
That outer operation returns an existing Failed cache entry directly. Reporting
subsequently enters `recursiveTypeRelatedTo`, where an ordinary Failed entry
causes recomputation instead of another replay.

The first call's silent comparison installs a clean failure. Its reporting
invocation replaces that comparison with the five-event completion. Therefore
the second call consumes five cached events during selection, before its
overload marker, and five more in the reporting wrapper, after the marker.
Its two displays still contribute five each.

| Phase | Missing events | Position relative to overload marker |
| --- | ---: | --- |
| First call: silent selection and reporting-wrapper cache read | 0 | Before/after first marker |
| First call: reported comparison display | 5 | After first marker; installed in relation cache |
| First call: outer relation display | 5 | After first marker; outside relation cache |
| Second call: selection cache read | 5 | Before second marker |
| Second call: reporting-wrapper cache read | 5 | After second marker |
| Second call: recomputed comparison display | 5 | After second marker; replaces relation cache |
| Second call: outer relation display | 5 | After second marker; outside relation cache |

Thus the cold result is `OverloadRecovery, MissingCache × 10`. The warm result is
`OverloadRecovery, MissingCache × 15, OverloadRecovery, MissingCache × 15`.
The warm total is four displays times five events, plus two failed-cache reads
times five events: 30 missing-type events and two overload markers.

The product counterparts are
[relation comparison/cache transport](../src/project/semantic/evaluation/relations.rs),
[reporting arguments](../src/project/semantic/evaluation/relations/reporting.rs),
[the optional elaboration wrapper](../src/project/semantic/evaluation/relations/elaboration.rs),
[call selection](../src/project/semantic/evaluation/calls.rs), and
[call reporting](../src/project/semantic/evaluation/calls/recovery.rs).
This accounting uses their existing per-invocation completion semantics, not a
new transitive cache-poisoning rule.

## Final assertion consequence

The public cause cap is 16 secondary events. Cold must have exactly ten missing
causes and no omission. Warm must retain fifteen missing causes followed by the
second overload marker, with exactly fifteen omitted causes. Both overload
markers retain their actual call ranges; every retained missing cause retains
the annotation range. The existing stage25 output has precisely this order and
count (`.cc/temp/stage25-integration-output.txt`).

The old unconditional no-omission assertion in
[the final Project test](../tests/type_diagnostic_cache.rs) was too strong.
The exact count/order/range assertions above are now staged, together with the
measured native acquisition/cache counts in the harness, for the next coordinated
validation. They do not deduplicate by location or suppress cached completion evidence.
The native observer does not directly expose product recovery counts; native
acquisitions plus the independently specified completion/cache policy establish
those counts. Native-members adversarial review confirmed this accounting.

This evidence resolves cause accounting only. Other cache/serialization
controls and the remaining complete reuse visitor remain required work.

## Generated private-key index recovery

The sparse generated-key control is
[native-generated-literal-keys/sparse.ts](../fixtures/types/native-generated-literal-keys/sparse.ts).
Its two profiles passed the maintained native baseline in
`vrun-93fbdd39-2c0f-44e2-b808-1b0c34138f50` (four groups including the original
24 roots; zero skips). Both compiler paths return `any`/errorType with native
2304 followed by 2339. Each sparse hook records 26 getter/cache observations;
there is one error-valued annotation read of `MissingNumberIndex`.

The final product cause count also includes reads of installed completion
slots. For a class with two names, the expected sequence is two failed index
inventory acquisitions from composite property/index lookup, four from the two
private-name candidates, two from the promised-type probe, one from diagnostic
object serialization, four from the cached suggestion inventory, and the
access's own missing-property error: 14 total. These are actual consumers of
cached evidence, not fourteen fresh compiler annotation evaluations.

The larger class has 13 names, so the two candidate-inventory consumers each
carry 26 events. Its sequence is `2 + 26 + 2 + 1 + 26 + 1 = 58`. The bounded
failure output preserves its primary event and 16 causes, with 41 omitted.
The sparse control independently makes the final own error visible.

Before correction, both final-query tests fail
(`vrun-c93e5923-409d-45a6-b798-8ccdb34c7816`): sparse stops after nine events;
the larger fixture stops after 31. `diagnostic_object` called the admitting
structured-members wrapper. Native `createTypeNodeFromObjectType`
(nodebuilderimpl.go:2663) continues after resolving those members. The corrected
owner sequences its completion through every serialization branch, preserving
both inventory recovery and subsequent work. Both final-query tests then passed
`vrun-4150c014-fcc4-4524-bda0-a9d2400d22d1` with zero skips, in both compiler
profiles and compact/expanded repeated query orders.
