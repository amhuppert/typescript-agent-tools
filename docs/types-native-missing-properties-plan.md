# Missing-property report ownership

The operating envelope remains one request-local native checker over a loaded
project. These operations reproduce semantic work performed while reporting a
query error; they do not add a project diagnostic command.

`reportNonexistentProperty` can acquire more than the missing member. Its union
walk, static-member probe, promised-type probe, diagnostic display, library
suggestion, spelling suggestion, never-intersection elaboration, and empty DOM
type probe belong to one evaluator operation. The access caller supplies the
original containing type and consumes one completion. A missing member does not
authorize skipping these dependencies or turning their failures into absence.

Keeping the private-access branch's original abbreviated display call would
duplicate policy and miss ordinary-access dependencies. Moving only individual
lookups into helpers would still make both callers know native branch order.
The implementation instead owns the complete report, with the native report-key
set and shared node `TypeChecked` flag installed before any dependency. This
keeps the ordering rule and reentry behavior in one place (Ousterhout P04/P10).

Two existing questions have actual second callers and therefore share owners:
`getSpellingSuggestionForName` serves property access and excess-property
reporting; `elaborateNeverIntersection` serves property access and relation
reporting. Each preserves native lookup order and completion provenance. The
feature map is a complete, ordered copy of pinned 7.0.2 compiler data, with exact
names and no configured-library inference.

The adjacent `checkAndReportErrorForExtendingInterface` gate must resolve the
heritage entity using the exact Interface flag. Looking up Type and filtering
afterward is incorrect: an inner type alias can be skipped while resolving an
outer interface. Its syntax walk is implemented. Its matching semantic branch
remains an explicit failure until the native-header resolver supplies exact
flag lookup; this temporary migration boundary is not delivered behavior.

Proof obligations are the final Q4 results and native acquisitions for static
and inherited static candidates, promise and thenable values, union/index
alternatives, library feature names and case differences, public/private/
protected suggestions, discriminant/private intersections, DOM names, and
interface-only heritage lookup. `native-missing-properties` contains these
inputs; the initial 34-root strict/loose capture is pending. Existing private
access diagnostics provide additional private-name and recovery controls.
