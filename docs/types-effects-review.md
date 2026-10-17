# Bounded call-effect review

This read-only adversarial Ousterhout review covers the initial
`evaluation/effects.rs`, Source call projection, and native Node::Call read/end
reachability routing. The envelope is one captured Project, one request-local
Evaluator and retained canonical bindings. TypeScript 7.0.2 commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b` supplies expectations.
No full call/new/tag resolution, apparent type, predicate inference or effect
interpretation gate is closed by this review.

## Concrete finding: inferred JSON property value is not an explicit dotted type

`explicit_dotted_type` currently returns `Some(establish_json(value))` for an
imported JSON value. For a named JSON property this acquires the actual module
dependency correctly, but substitutes its inferred value for the separate
`getExplicitTypeOfSymbol` question. The latter returns nil for a Property symbol
whose own declaration lacks an explicit annotation. The erroneous Some(number)
then demands an apparent primitive wrapper and refuses an otherwise healthy
read as unsupported.

The exact control is:

```typescript
import { count } from "./data.json"; // { "count": 1 }
let healthy = 1;
count();
healthy;
```

Fresh pinned API and same-checker hook both return number/errorType=false for
the final healthy identifier. The hook has only initialization diagnostics
2318, with no consumed call error or wrapper recovery. Research artifacts are
`.cc/temp/types-effect-json-review/{read.ts,data.json,tsconfig.json,answers.json}`.
The final Project reproduction is `tests/type_effects_review.rs`.
Its first registered run `vrun-9e06ef89-0e7a-41cb-a3b2-d05efc6f5f3c`
failed compiling the concurrently staged effects owner's nonexistent
`demand_type` calls; this is not a behavior RED.
After those mechanical typos were corrected, registered run
`vrun-93e43136-cd7a-48bc-adac-d471506f2d76` compiled and executed the
single final-boundary test, failing the expected healthy Number assertion with
`UnsupportedSemantic: native apparent primitive wrapper call signatures are
pending` at the JSON call. This is the behavior RED for the finding.

The source-backed remedy keeps alias/module acquisition, then establishes nil
for the admitted named JSON property's absent annotation. Binder.go
`bindSourceFileIfExternalModule` (759) creates the JSON export= Property;
checker.go `getTargetOfImportSpecifier` (14628) resolves a named property through
the entire root type before retrieving its property symbol; flow.go
`getExplicitTypeOfSymbol` (2134) returns nil for an unannotated Property. Do not
skip module dependencies. A BoundJson root alone still lacks the default versus
namespace alias flavor, so that neighboring operation remains explicit pending.
This corrects a semantic question boundary without changing lexical reporting.
The owner now acquires the actual JSON root and returns known absence for the
named property's explicit-annotation question. Root's registered reproduction
GREEN `vrun-867f48a7-a66d-442f-b6a0-5ff5227855cb` passed the single test
with zero skips. The neighboring root/default/namespace flavor obligation is
unchanged; its existing construction guard remains explicit.

## Dotted syntax hypothesis and the actual binder boundary

`getTypeOfDottedName` recursively unwraps parentheses, handles
Identifier/This/Super/Property, and returns nil for every other syntax kind.
The evaluator's current Identifier-only operation looked narrower. Tracing its
actual caller changed the assessment: native
`maybeBindExpressionFlowIfCall` and the retained builder create a FlowCall only
for dotted callees, and the existing Source expression projection unwraps
parentheses. Non-dotted callees therefore never reach this effect operation
in the currently admitted read/end-reachability family. No production change
is justified by that initial hypothesis.

Registered fresh compiler controls
`vrun-df210a2e-da55-45a3-9ca2-a7524663814c` execute two tests with zero
skips: ten syntax variants under strict/loose, plus same-checker
element/arrow/nested-call final demands with no callee-type/call diagnostic
consumption. An earlier control `vrun-f448dffe-4ccc-47e7-9e0b-f451b0db0dd1`
passed the API matrix but failed a wrong hook DTO field name; it is not a
behavior RED.

Final Project run `vrun-c671bd35-a7ff-49d8-96d3-808562f79985` passed the
JSON control and six preceding callee variants, then failed the conditional
variant's existing `FlowRequired` binder graph obligation at the later read.
That is a missing ConditionalExpression flow operation, not an effect lookup
failure. The compiler expectation remains recorded, and full native graph
support remains mandatory. The final matrix keeps the conditional case as the
behavior RED for the actual binder owner. Root owns that whole conditional flow
schedule; no effect-specific workaround or permanent unsupported outcome is
introduced.
Root completed the conditional's actual binder schedule, including restoration
of the original flow when neither branch has effects. Extended compiler GREEN
`vrun-7c591142-5e52-4376-89b9-ee6b7890b0aa` and final Project GREEN
`vrun-a00a259b-ba76-4b95-9ab6-e945290fdd0f` both execute their two-test
files with zero skips. The final matrix keeps all ten syntax forms and adds
pure matching-typeof, pure unknown-condition, nested pure conditional and
actual branch assignment cases under strict/loose. These support the full
conditional schedule and preserved call-question boundary for this slice;
they do not prove optional/logical/try/switch flow families.

## Other reviewed boundaries and limits

The operation preserves unknown raw symbols as known absence for explicit
dotted acquisition; failed/unvisited bindings remain failures. Ordinary
unannotated variables return absence before checking an initializer, and
annotated variables/functions use their actual canonical value operation.
Source's exact call role and flags remain metadata; signature choice belongs to
Evaluator. Expression-statement dotted checking and nonstatement ordinary
checking are distinct native questions. The current admitted Node::Call family
is expression statements; optional/property/this/super/primitive-wrapper and
constraint/composite families retain explicit pending dependencies.

Single nongeneric signature selection and ordered possible-effect scanning
follow flow.go `getEffectsSignature` (2026). A possible overloaded/generic effect
retains the real getResolvedSignature obligation; no-effect calls skip argument
and body checking. Explicit predicates demand their asserted type; annotated
return questions preserve their actual deferred/mapper identity. Body inference
with parameters remains a FlowRequired dependency. Native getParameterCount
also understands tuple-rest counts, so raw parameter vector length cannot become
a universal predicate-inference rule when that pending family is admitted.

The request cache distinguishes resolving, resolved Some/None, and failed
DemandError. Reentrancy stays an explicit failure, and neither a failed target
nor a failed return/predicate demand installs a no-effect entry. Exact located
failures are cached unchanged. Native assertion/never interpretation remains a
separate pending read/reachability operation after a known effect; absence
continues through the actual antecedent. No global diagnostic list is used to
classify arbitrary selected roots.
