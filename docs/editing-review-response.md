# Editing design review response

Accepted all four findings and revised [the E/E1/E2 design](editing-design.md).
The review correctly identified contracts that were too implicit inside the
proposed hierarchy. The revisions specify those contracts without adding a
validation framework or prescribing a class for each responsibility.

This responds to the review in conversation `next step 5`, assistant message 1,
2026-10-08 at 22:17 UTC. Retrieve it with:

```sh
cctl conversation read 8a86a8c2-3c32-4171-8b17-b97e756815fb --message 1
```

The baseline remains `ac24bc5`, with one active writer during apply. This is a
design revision; no production behavior has been implemented or fixed.

## 1 Structural preservation

**Accepted.** “Ignoring derived flags” was an unsafe generalization. The parser
sets `DEPRECATED` from collected JSDoc, while an allowed edit can change the
source-file dynamic-import/import-meta summaries. Those need different treatment.

The [structural preservation contract](editing-design.md#structural-preservation)
now compares syntax fields, exact values, and meaningful flags by default and
names the exceptions. It preserves `DEPRECATED` and contextual/literal attributes;
allows the two named source-file summaries to describe the new file; checks
`SourceFile.text` through composition; and leaves parser bookkeeping, compiler
caches, and the `assertClause` alias with the existing AST contract and owner.
Unknown metadata does not inherit a blanket exclusion.

The design walks through E1 opening only an arrow's body within a multi-binding
statement, and E2 opening a gap between existing statements. Both use the change
map to relate protected fields, tokens, and siblings. Operation policy owns what
may change, shared comparison owns correspondence, and context validation runs
the checks. This addresses the risk of a context validator accumulating every
editing rule.

## 2 Trivia obligations after composition

**Accepted.** A planned seam and equal syntax subtrees cannot establish comment
preservation. The AST does not contain comment directives or full JSDoc trees;
unchanged comment bytes can govern different code after insertion.

The [trivia contract](editing-design.md#trivia-and-layout) now makes the
attachment owner plan and verify relationships on the final composed source.
It checks existing protected relationships and effects from both fragment ends.
New directives or JSDoc must not affect preserved syntax outside the opened
region. Harmless trailing comments remain valid. Context validation invokes this
operation without interpreting attachment policy itself.

The ownership change covers the complete decisions already present in the code:
`pragmas` recognizes directives and intervening lines;
`project::program::diagnostics::Suppressions` owns coverage under
`getDiagnosticsWithPrecedingDirectives`; and parser `deprecated_trivia` contains
the `iterateCommentRanges` collection rule. The relevant coverage/collection
operations become shared when editing needs them, with existing consumers routed
through them. The narrower CR/LF JSDoc collection rule is preserved. This does
not require a second scanner or full JSDoc parser.

## 3 Saved plan construction

**Accepted.** Revalidating saved obligations would create another representation
and another route that must establish editing eligibility. The revised
[plan contract](editing-design.md#prepared-batches-and-failure-semantics) uses
deterministic replay instead.

Save original images, analysis filenames, resolved site anchors, operations,
embedded fragments, proposed images, and physical identities. Loading checks
shape/build and reconstructs preparation and binding against those originals.
The operation and attachment policies recreate their private obligations, and
the resulting proposed bytes must exactly match the saved output. No serialized
obligation or “validated” flag establishes trust.

Replay does not reload a project, reread fragment files, or resolve names in
current sources. Current physical-file checks remain part of apply. The
[complete editing boundary](editing-design.md#complete-operations-and-existing-analysis)
explicitly owns batch invariants, save/load, and preview derivation; preview and
apply consume the same prepared images.

## 4 Binding analyzed text to files

**Accepted.** The distinction existed in prose but lacked a complete operation.
The design now names a **validated text proposal** and **local file binding**.
One binding operation checks decoded-text equality and maps changes into the
captured raw image through the source owner. File mode can capture once; project
and service modes capture selected files after receiving the proposal. All modes
use the same equality, mapping, and batch checks.

Selected-file analysis must lend the AST and projected syntax identities during
their valid lifetime. The current file-navigation operation explicitly drops its
AST before invoking its answer callback. That owner will be extended for editing;
individual operations will not compensate by reparsing excerpts, choosing parser
budgets, or implementing BOM offsets. AST identities stay out of transport and
saved plans.

## Additional refinements

| Review point | Disposition |
| --- | --- |
| Separators and layout | Accepted. Separator policy considers both fragment ends and the containing list. The design permits at most one preliminary candidate parse to discover protected literal spans, followed by final validation after layout. If safe rebasing is unavailable, preserve supplied bytes and validate; there is no parse/reformat search loop. |
| E2 and Q2 statement designation | Accepted. Share the underlying designation; E2 adds direct membership in a supported statement list. Rendering consumes it. Q2 retains its ability to show loop declaration lists that are not E2 slots. |
| Downward parentheses traversal | Accepted. Native `SkipParentheses` is parentheses-only and differs from upward `WalkUpParenthesizedExpressions`. E1 and projection's existing downward loop should use one owner. No broader stripping of assertions or wrappers is introduced. |
| Batch and per-file lifecycle | Retained and clarified. The per-file owner hides valid combinations of target state and recovery resources; the batch owner derives the reported effect from its outcomes. |
| Syntax versus binding guarantees | Accepted explicitly. Adding an export may change binding and still be allowed. A changed parse of protected code is rejected. No semantic-equivalence check is added. |
| Internal state and further subdivision | Retained. Parents and siblings should need fewer decisions, not merely fewer lines. Broad shared mutable state would defeat the hierarchy; private operations suffice where their contracts hide coherent knowledge. |

## Verification

Source inspection confirmed the relevant ownership and lifetime facts in
[parser](../src/parser/mod.rs), [AST](../src/ast.rs),
[projection](../src/navigation/projection.rs),
[file navigation](../src/navigation/file.rs), [source](../src/source.rs),
[pragmas](../src/pragmas.rs), and
[diagnostic suppression](../src/project/program/diagnostics.rs). The native
7.0.2 AST utility source confirms the two parentheses operations are distinct.

Fourteen focused source variants were run through the current Rust parser,
TypeScript 6.0.3 syntax API, and TypeScript 7.0.2 with and without `--noCheck`.
All 14 parse; their differing outcomes show why parsing alone is insufficient:

| Probe | Observation |
| --- | --- |
| Trailing `/** @deprecated */` before an old function | The old function's flag changes from 0 to `536870912` in both syntax projections. An ordinary trailing `//` comment leaves it at 0. |
| Insert a statement between an old `@ts-ignore` and its target | The native compiler changes from no error to TS2304 on the preserved target. |
| Add a fragment-edge `@ts-ignore` before a formerly unprotected target | TS2304 disappears without changing that target's ordinary syntax fields. |
| Replace a body with a dynamic import or `import.meta` | The source-file summaries become `4194304` or `8388608` in both syntax projections, while the other function remains unchanged. These are legitimate representation changes. |
| Insert `export {}` | The compiler's source-file module indicator changes while the existing ordinary declarations remain syntactically unchanged. |
| Add a module indicator beside `await(foo)` | The preserved expression changes from `CallExpression` to `AwaitExpression` in both syntax projections. This is a structural change, regardless of the new file passing a normal compiler check. |

Session evidence is in
[the probe script](../.cc/temp/editing-review-probe.mjs) and
[complete inputs and results](../.cc/temp/editing-review-probes/results.json).
These probes support the design choices; they do not test an implemented editing
engine. No production test suite was rerun for this documentation-only revision.

The design's proof obligations now require both final-file and plan-load cases
for escaped/retargeted comments, meaningful metadata, allowed source summaries,
and changed parses of preserved code. Replay must also reject ineligible targets
and altered proposed images while remaining independent of current project and
fragment contents. Those remain implementation evidence to produce with
behavior-level red-green tests.
