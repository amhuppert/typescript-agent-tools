# Deprecated signature checking

This is a native semantic dependency closure for the existing checked call,
new and tagged-template owners. The operating envelope is the current TS/TSX
captured-source query worker with a shared request resource control. It does
not add diagnostic output or a JavaScript/JSDoc type-system input profile.
The implementation is being checked in coordinated source/validation stages.

## Native operation and evidence

Pinned checker `checkDeprecatedSignature` (8340) first rejects
`IsSignatureCandidateForOverloadFailure`, then tests the selected signature's
actual nonnil declaration with `IsDeprecatedDeclaration`. On success it obtains
the suggestion node and invoked-name text, calls `signatureToString`, and adds
the suggestion with the actual declaration as related-information input.
The display is a semantic operation: it can acquire previously unread types.

`checkCallExpression` calls this operation after the resolving-signature
sentinel and before Super handling, new-on-call-signature recovery, or return
acquisition (8298). `checkTaggedTemplateExpression` does so before its return
acquisition (10015). The same owner must serve `checkDecorator` (6043) when that
producer is admitted. Quick `getSingleSignature` return acquisition does not
perform this check.

The isolated `native-deprecated-signatures` fixture has 23 queries across
strict, loose and exact-optional profiles. Capture receipt
`vrun-836bbb45-0080-4d2f-bac5-925b4ff08e0f` established all 69 results; the
maintained harness asserts them in GREEN receipt
`vrun-094eeaf4-115e-47af-8067-9c52f1c655c9` (three tests, zero skips).
It inspects the hook's **before-summary** observations, so the public summary
cannot cause the observed dependency.

* A checked nondeprecated call skips the omitted optional parameter's
  `{ bad: MissingNormal }`; its deprecated twin consumes `MissingDeprecated`.
* A nested optional function parameter with `T = Missing` is likewise skipped
  ordinarily and consumed by deprecated display. The hook reports the actual
  failed type-parameter default observation.
* A properly attached constructor JSDoc tag consumes the hidden parameter's
  bad property. A quick initializer query of that constructor skips it.
* In strict mode, `new` on a nondeprecated call signature returns native Any
  without reading its bad return. The deprecated twin reads that return for
  signature display before the same Any recovery. Loose mode reads both returns
  during native constructor resolution, so it is a distinct expected variant.
* Only the selected deprecated overload consumes its hidden dependency; a
  healthy deprecated signature stays healthy. A tagged template also consumes
  its selected deprecated signature's hidden dependency.
* Inline code, fenced code, email text, a longer tag name, a hyphen or NBSP
  directly after `deprecated`, and inline link text do not activate the check.
  An actual second tag following `@param` does activate it.

All final native root types remain healthy in these controls. Product failure
expectations belong to the consumed original annotation/default/return error,
not to a fabricated deprecated-signature error or native root errorType.

## One declaration decision

The current `node_flags::DEPRECATED` bit belongs to the **TypeScript 6.0.3 JSON
syntax projection** (`src/compiler.rs`, bit 29). Native TypeScript 7's
`NodeFlagsPossiblyContainsDeprecatedTag` is bit 26. They must not be substituted
for each other, and neither the generated public bit nor the existing editing
comparison may be changed as a shortcut for this semantic operation. Retain a
private native JSDoc attachment/header fact at the actual parser factories;
serialized AST flags remain baseline-compatible. Native
`IsDeprecatedDeclarationWithCachedFlags` (ast/utilities1223) checks the combined
flags, then walks ancestors to the first directly flagged node and asks that
node's lazily parsed `GetJSDocDeprecatedTag`. The first flagged ancestor's
negative result is final; it does not continue searching.

There are two viable ownership placements:

1. Eagerly parse every potentially relevant comment while building the AST and
   retain an additional confirmed fact. This keeps evaluator reads simple but
   adds parsing and allocation to files whose JSDoc is never queried, and the
   existing possible flag must still remain unchanged for AST compatibility.
2. Keep the native attachment/header fact and put complete JSDoc tag acquisition behind one
   lazy captured-syntax operation, cached by actual node identity. This follows
   native demand timing and reuses the existing Source syntax/resource owner.

Choose the second, with native's eager exception described below. A Source operation such as
`deprecated_declaration(DeclarationIdentity) -> Result<bool, DemandError>` owns
combined flags, the ancestor walk and actual tag acquisition. A separate
`jsdoc_deprecated_tag(DeclarationIdentity)` operation returns an actual tag
range for the native related-information question; that question uses the
original declaration, not a substituted ancestor. Neither is a symbol-name or
display-string lookup. Source should share the tag cache for the two questions.

The shared lexer `jsdoc_comments` already owns comment-range collection and its
narrow CR/LF rule; `syntax::trailing_jsdoc` owns which kinds also collect trailing
comments. Reuse them. Add the complete native
`scanJSDocCommentForTags`/`hasJSDocTag` question (scanner348): only end, ASCII
space, tab, CR, LF, `}` or `*` terminate the cheap spelling. This question retains
both the deprecated and see/link scanner facts. The current `mark_deprecated`
uses a broader `!id_continue` test for the legacy projection; do not silently
replace that projection while adding semantic metadata.

Do not infer native attachment from a node kind plus nearby comments. Capture
it where the parser performs native `withJSDoc`, using that operation's saved
scanner position and facts. Existing Rust attachment calls cover statements,
parameters, class/type members and enum members; native also attaches to
function/constructor types, arrows, function expressions, object-literal
elements, variable declarations, export specifiers and other explicit parser
sites. Migrate the complete factory-site inventory before claiming this shared
metadata owner complete. Source's combined-flag traversal must reuse the
existing `getCombinedFlags` owner with native header facts, not copy its
binding-pattern/variable-list/statement ancestry rules.

For TS/TSX, native `withJSDoc` normally leaves the tags unparsed. A preceding
see/link scanner flag triggers parsing immediately; a parsed deprecated tag
then ORs into the native possible flag even if the cheap deprecated scan did
not match. Preserve this exception and reuse its parsed facts in later Source
queries. The final flags cannot be reconstructed from the legacy serialized
bit. Lazy results, including interruption, belong to the actual node's one tag
cache; no evaluator cache or symbol-name cache is needed.

Lazy confirmation requires the complete native tag-boundary decision, not a
second substring search. The shared JSDoc parsing operation must preserve
`parseJSDocCommentWorker`, `parseTag`, link and tag-body advancement, including
inline/fenced backticks, whitespace and line rules, incomplete tags and nested
tag bodies. Reuse normal parser type/name operations where the native JSDoc
grammar actually does. It may retain only the tag facts/ranges needed by current
consumers instead of exposing a new public JSDoc tree, but cannot omit grammar
advancement that changes which tags are top-level. All loops/allocation and
speculation must use the existing opt-in resource control; interruption remains
a sticky whole failure, not an absent tag. A partial tag classifier is not a
completed prerequisite.

Native discards JSDoc parse diagnostics in TS files. Malformed JSDoc therefore
needs the native parser's recovery and advancement, rather than converting a
tag grammar mismatch into an ordinary source-input failure. Resource/deadline
interruptions remain whole failures and are never rewound with syntax or
discarded diagnostics. Parsed facts may retain exact tag kind/range without
exposing a public JSDoc tree; any temporary syntax allocation still counts
toward the shared request resource ledger.

## Evaluator integration

Add one `check_deprecated_signature` operation in the signature owner. A typed
`SignatureFlags` getter performs the failure-candidate guard before any Source
question. Obtain the existing `signature_declaration_root` and its actual
`DeclarationIdentity`; never reconstruct a declaration from the callee, name,
type origin or alias. Keep its exact declaration location available for Source
and related information. A nil declaration is an ordinary no-op.

After confirmation, the only signature display call is the existing
`diagnostic_signature_display(&Signature, &Location)`. This is native
`signatureToString`: a fresh node-builder scope, flags None, nil enclosing node,
and no type-display serialization-level increment or final byte truncation.
Its internal `signatureToStringEx` entry (printer342) always adds node-builder
`IgnoreErrors | WriteTypeParametersInQualifiedName`, even though the public
TypeFormatFlags argument is None. Retain those internal policies in the shared
renderer; its current `Context(0, None, false)` does not yet represent the latter
qualified-name policy. The native symbol-chain branch can acquire enclosing
generic parameters (nodebuilder1039/3541). Add a qualified generic signature
control when closing that renderer dependency; the 69 ordinary controls above
do not establish it.
Keep the internal node-builder flags separate from the caller's TypeFormatFlags.
`enterNewScope` does not create an enclosing declaration when the entry's
enclosing declaration is nil, so an ordinary qualified source spelling alone
does not prove the generic-chain branch. Controls must exercise an actual
native context transition/chain and its observed parameter acquisition. The
shared display-scope owner must serve both signatures and mapped types:
expanded/original parameters, type parameters and mapper are its inputs;
name-map scopes, injected locals and restoration on failure stay internal.
Do not call Q4 summary, type display, or invent a declaration-enclosing argument.
The signature still retains its actual declaration for native node-builder
reuse and parameter metadata. Use the actual checked expression location as
the operation consumer; annotation/default failures keep the independent
original Location already carried by DemandError.

Wire this operation at the native positions above. A display failure must
survive any installed resolved-signature cache and must stop before the later
call-only Any/return path. Do not add a global deprecated-signature cache:
native calls perform the operation each time, while declaration/tag and type
dependencies use their existing caches. Syntax-only suggestion/name/related
information can be acquired by Source without adding public diagnostics.

## Proof and implementation boundary

The first coherent implementation is the shared possible-tag correction,
complete lazy tag acquisition, and the thin signature operation wired into all
currently admitted invocation producers. Keep quick queries unchanged.
Maintain final Rust controls corresponding to the measured matrix, asserting
healthy types for negative gates and exact consumed-failure reason/location for
positive gates. Add attachment CR/LF/CRLF/Unicode-line and punctuation/end
variants derived from the two distinct native scanner rules, declaration-kind
and inherited combined-flag cases, plus resource interruption inside long
comments. Prove the final tests fail when the signature invocation is disabled
and separately when actual tag confirmation is replaced by the possible flag.
Recovered overload-failure signatures need a real factory/control before
claiming their candidate-bit branch proven.

Native fixtures alone do not establish implementation completion. The JSDoc
prerequisite and qualified signature renderer remain part of this operation's
required work.

## Source ownership and checkpoint

The implementation owns private native JSDoc header storage in `ast.rs`, the
scanner question in `lexer.rs` (or its JSDoc submodule), complete tag grammar
in `parser/jsdoc.rs`, and the parser factory attachment calls. The public JSON
projection is unchanged. `source/deprecated.rs` owns lazy acquisition, actual tag
ranges and declaration confirmation; `source.rs` only registers its state.
`evaluation/signatures/deprecated.rs` owns `checkDeprecatedSignature`, with the
actual candidate-bit observation in `SignatureFlags`. Existing checked call,
new and tagged-template dispatch calls it at their native positions; quick
return acquisition remains a separate operation.

Renderer work stays in `evaluation/native_display`: the shared scope owner
(coordinated with mapped serialization), internal signature-entry flag, and
the actual symbol-chain/qualified-parameter operations. It must not be a
deprecation-only display path. This part and the isolated native/final control
authoring can proceed independently of the JSDoc parser after their interfaces
are fixed. Parser/Source shared files and invocation dispatch require narrow
coordination windows. The first integrated checkpoint compiles those owners
together, then runs the measured deprecation matrix plus AST/editing and
resource regressions; a parser-only milestone is not deprecation completion.

## Current coordinated checkpoint

The private scanner header, actual parser-factory attachment, lazy Source tag
cache, complete tag-body orchestration, and checked-call operation are staged.
They preserve public serialized flags. JSDoc recovery changes stay inside the
shared parser operations and its private JSDoc mode; they do not turn resource
interruptions into grammar recovery. Scanner keyword classification now has
one owner, with distinct Identifier/reserved/contextual keyword partitions.

The new 50-query capture across all three profiles is
`vrun-d689704d-8427-43c7-8d50-5b4d4354128f`. It proves eager see/link effects,
escaped tag-name differences, tag-body advancement, and declaration-family
eligibility. Its maintained assertions also observe the actual failed hidden
property or type-parameter default before summary. The initial generated CR
and CRLF cases were normalized during text rewriting; their actual bytes are
now restored. The 150-observation asserted rerun hit the server's 900-second
budget (`9789d961`), so the maintained harness now has one registered file per
profile, with one shared assertion owner. This is a runner limit, not native
semantic evidence. The separate malformed-recovery fixture retains literal
CR/LF/CRLF bytes and all 39 assertions passed in `9af7b4ed`.
All 150 maintained assertions now pass in the three bounded profile runs:
strict `03b2736f`, exact-optional `d76e2dfe`, and loose `89a364a4`. The method
case retains TS2304 but has no failed-property observation: its annotation is
consumed by method checking before the serializer's property getter. The
assertions preserve that measured distinction rather than treating every
deprecation dependency as the same getter event.

Eager parsing preserves the live parser's context flags; lazy
`parseJSDocForNode` initializes a fresh parser. The four-case, three-profile
capture `e909df30` distinguishes `@see await@deprecated` inside an ordinary
function from its async counterpart, and similarly distinguishes ordinary
`yield` text from a generator context. Only the ordinary cases consume the
hidden annotation. The shared fragment worker now accepts its caller's actual
context for the eager entry; lazy callers keep the fresh-context behavior.
The maintained twelve-observation assertion run passed in `8b0b5faf`.
Final Project controls assert those exact consumed annotation locations and
the healthy async/generator results. The previous integrated `type` failure
was the test searching for the prefix `Missing_type` inside `Missing_typedef`;
the assertion now identifies the complete fixture token.

The real qualified-name witness uses a static unique-symbol property
`Keys.key`, with `class Keys<T = MissingQualifiedDefault>`. Rendering an omitted
optional parameter containing that key under `signatureToString` consumes the
class default; an ordinary call to its otherwise identical nondeprecated twin
does not. The native root type remains healthy in every profile. The final
Project test therefore requires the exact original unresolved-default failure,
not a generic display failure.

That branch still requires the shared unique-symbol producer, property-name
context transition, accessible symbol-chain owner, and actual
`lookupExpressionChainTypeArgumentNodes`/`lookupTypeParameterNodes` operations.
The internal `WriteTypeParametersInQualifiedName` flag has been retained, but
retaining the flag is not a substitute for those operations. The shared scope
owner is coordinated with mapped serialization. Nonnil synthetic lexical
scopes must retain actual parameter symbols; no fabricated parameter identity
or deprecation-only renderer is permitted.

The stage-16 display checkpoint publishes the ordinary nil-context
`symbolToTypeNode` path and one shared `getSpecifierForModuleSymbol` operation
for that entry and standalone symbol display. ImportType remains distinct from
TypeQuery in output syntax and Q4 precedence. The nonnil/FullyQualified symbol
chain, fake-scope locals, and contextual `modulespecifiers.GetModuleSpecifiers`
algorithm remain explicit required dependencies. Source scope facts and the
captured/synthetic scope identity now have an owner, but those facts alone do
not implement qualification.
