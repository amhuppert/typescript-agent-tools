# Directed member reference search

This implements Q3 inside the existing immutable Project and request-owned
Evaluator. The operating envelope is the current local CLI/service: searched
project files are captured before a query, dependency files may supply origins,
and candidate checking must not reopen files or publish internal symbol IDs.
The full contract remains in [the design](types-design.md#q3-references-relationships-and-calls).

## Ownership and alternatives

The current lexical index owns identifier tokens, declaration-name lookup and
the final lexical projection. Reusing its declaration-name lookup for member
search misses literal accesses and ordinary uses. Building a second semantic
token table would instead duplicate source ranges, exact-name conversion and
selection precedence. Extend the existing collected names and frozen occurrence
table with the missing name-bearing roles. Keep lexical seed lookup and decoded
candidate lookup as distinct indexes over those same entries: they answer
different questions. Binding captures syntax and exact keys; reference search
decides candidate eligibility and meaning using the native LS operations.

The current property-symbol owner already distinguishes canonical, instantiated
and created symbols. Reference search must consume a shared `GetRootSymbols`
operation there. It must not infer roots from public origins, structural
assignability, type equality or the `PropertyDisplay` shape. Synthetic union
roots ask the constituent property lookup; transient roots use the native
spread, mapped origin and target precedence. The related existing
`forEachProperty` operation has a different callback/short-circuit contract and
cannot silently stand in for this question.

One request-owned search engine belongs beside evaluation and presentation.
Its external operation selects a seed, creates its directed search set, visits
eligible candidates in exact normalized path/start/end order, and produces an
owned result. Putting relationship policy into binding would force the loader
to evaluate types and would change legacy lexical behavior. Putting it into
presentation would make semantic identity depend on graph/display budgets.

## Native operation boundary

Selection retains source-node identity and role, including lexical parameter
versus parameter-property member and shorthand lexical precedence. The selected
seed runs the established outer-to-inner checking prelude, then its actual
symbol operation and combined-member normalization. Dotted selectors reuse Q2
catalog sites; canonical symbol and question identity decide ambiguity before
public candidate projection. Undotted names and lexical position roles retain
the existing lexical operation and its named coverage baseline. Actual member
declarations/accesses and the parameter-property modifier anchor invoke member
search. A dotted namespace declaration replays the same lexical operation and
symbol as its actual segment position. Raw LS shorthand/parameter queries can
return a broader family; preserve those observations separately from this
documented lexical operation, rather than silently changing its completeness.

The reference engine owns `getReferencedSymbolsForSymbol`,
`skipPastExportOrImportSpecifierOrUnion`, `populateSearchSymbolSet`,
`forEachRelatedSymbol`, `getRelatedSymbol` and their local search state. Port the
actual ordered native branches rather than collecting all apparently related
declarations. In particular:

- Inline TypeLiteral union declarations substitute the union property only at
  the native all-constituents-have-property gate. Interface union seeds retain
  their separate directed behavior.
- Contextual object declarations, shorthand values and destructuring follow
  their selected-site order. Successful matching can skip later dependencies.
- Root and base/implemented-interface symbols remain distinct. Static and
  instance checks and the explicit-inheritance filter precede a base match.
- Parameter-property symbol pairs and export-specifier local targets use the
  existing canonical binding owners. UMD/namespace cases retain native alias
  identity instead of comparing displayed names.

Evaluation owns symbol-at-site, combined-member resolution, contextual property
queries, root symbols, declared bases and property acquisition. Missing native
producers remain located failures at their actual demand until implemented;
they are not empty symbol sets. Reference search sees typed symbol handles and
these operations, not type arena layout or syntax-node allocation details.

## Coverage and result boundary

Candidate eligibility ports `getPossibleSymbolReferencePositions`,
`isValidReferencePosition` and `getMeaningFromLocation`, with the design's exact
decoded-key normalization. Nonliteral computed expressions are not wildcard
candidates. The native `getSymbolScope`/`getReferencesInContainerOrFiles` domain
comes before candidate checking: a private member's containing class can bound
its search, so unrelated same-key tokens outside it cannot create gaps.
`getReferencesAtLocation` owns import/export/shorthand handling and the native
continuation to import/export searches. Port branches reachable from member
seeds and related candidate symbols; do not apply constructor/class special
searches to a seed retained by the lexical operation. A proven `any` receiver without a symbol is an established
non-reference; unknown/recovery dependencies are not. Call/test filters apply
before candidate checking through the existing syntax and path-policy owners.
The selected seed alone supplies the directed search set.

The ledger records each eligible candidate as established reference,
established non-reference, or a located gap. Unsupported and invalid candidate
dependencies become gaps; the same failures at the seed abort selection.
Resource, I/O and internal failures abort the whole operation. Exact candidate,
reason and cause identity owns deduplication; bounded causes retain an omitted
count. Diagnostic rollback never erases a consumed semantic failure.

Groups use actual matched symbol identity, retain its complete declaration
set, combine occurrence roles, and sort by exact source locations. A candidate
may belong to multiple directed groups. Keep the seed group even when filters
remove all its occurrences. There is no successful output truncation.

Add `ReferenceAnswer::Selected | Unselected` to the new request boundary;
preserve legacy `find_references` and `ProjectRequest::References` result types.
The CLI routes new requests through the typed answer and existing delivery
owner: partial and unselected JSON are delivered results with exit 1. Session
and service use the same dispatch, including refresh and captured dependencies.

## Proof and order of work

1. Capture native baseline and normalized observations for the core fixture:
   generic roots, unrelated equal members, both union seed forms, escaped
   identifiers, numeric/literal names, genuine any and unknown candidates.
   Keep native hook source frozen throughout each registered run.
2. Extend the shared inventory and symbol-root metadata, preserving lexical
   fixture, JSON and retained-binding tests. Add final Q3 RED controls before
   implementing the search operation.
3. Implement selection, relationship traversal and complete candidate ledger
   as one usable public path. Assert final Project and CLI answers against the
   compiler observations, including all option profiles the native rule splits.
4. Add contextual/shorthand/binding/parameter-property/base/spread/mapped/private
   and JSX matrices as their native producers become available. Exercise cold,
   warm and reversed selection order explicitly. A partial checkpoint does not
   narrow the deliverable.
5. Verify call/test filtering, gap order/causes, empty retained seed groups,
   dotted ambiguity, direct/service equality and request-generation isolation.
   Re-run lexical compatibility and the fixed corpus after the semantic path
   is complete. Adversarial review checks false healthy answers, missed
   candidates and unjustified relationship merging, not just positive matches.

The initial core capture is staged in
`tools/type-member-search-core-oracle.test.mjs`; capture alone is not conformance
proof. Final assertions and receipt identities are added after observations.
