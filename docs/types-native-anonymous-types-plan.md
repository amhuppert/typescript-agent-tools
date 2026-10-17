# Native anonymous type nodes and alias display

This increment uses the existing immutable Project, canonical Store symbols
and request-local Source/Evaluator. It completes the shared native
`getTypeFromTypeLiteralOrFunctionOrConstructorTypeNode` factory for its three
syntax families. It introduces no public testing accessor or second binder.

The earlier FunctionType/ConstructorType path constructed captured structural
signatures directly, while TypeLiteral uses the retained node symbol and its
member table. Keeping those paths would repeat anonymous identity, alias and
inventory decisions whenever a new signature family is admitted. The chosen
design extends the existing node-symbol factory and canonical member projection
to function and constructor types, then deletes the parallel structural path.
Source captures exact syntax and alias facts; the binder owns both symbols,
and the evaluator owns allocation, native node-link caching and member demand.

## Compiler facts and retained roles

At TypeScript 7.0.2 source commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`,
`bindFunctionOrConstructorType` creates a Signature symbol first, adds the own
syntax node to its declarations, then creates a TypeLiteral symbol and adds
the same node to that symbol. The second `addDeclarationToSymbol` is the final
`node.Symbol` attachment. The TypeLiteral member table contains the distinct
Signature under the typed reserved Call/New key. The Signature has no native
parent symbol. Its signature declaration is that same syntax node.

Eight disposable fresh API/same-checker controls (ordinary function,
constructor, generic function and abstract constructor, strict and loose)
confirmed these associations. The exact internal symbol/key bytes were
observed separately from message strings: anonymous type `fe74797065`, call
`fe63616c6c`, new `fe6e6577`. Final node/type symbol flags are 2048; reserved
member flags are 131072. Public `GetSymbolAtLocation(typeNode)` returns nil,
which is a different question and does not establish native node.Symbol nil.
Scratch code and answers are `.cc/temp/types-anonymous-signatures/`.

The registered native suite `tools/type-anonymous-types-oracle.test.mjs`
passed three tests, zero skipped, in
`vrun-454929b3-2a93-4d08-9088-1092bcc94e88`. It independently observes the
actual TypeLiteral symbol, distinct Signature member, shared own AST node,
absent member parent, call/construct partition, generic and abstract summaries,
and alias canonical/display name distinction through fresh API snapshots.

The collector will retain both own declaration associations and explicitly
identify the overwritten intermediate attachment. Existing canonical symbol
lookup will select the final node-symbol association; it will not merge the
two identities or reconstruct them from public origins. The linker owns the
Signature-before-TypeLiteral allocation and reserved-table attachment in one
operation, using typed keys. The source-member projection will consume each
reserved association's actual syntax kind, including FunctionType and
ConstructorType. Function signature parameter scopes retain their existing
native local-scope owner.

## Complete node factory and alias facts

The evaluator's node factory consumes actual `getMembersOfSymbol` table
presence before choosing the shared empty-type-literal identity. A nonempty
function/constructor signature table cannot select the empty branch. It
installs the anonymous identity before demanding signature types and shares
the existing actual inventory question. Direct signature/constructor token
positions remain excluded by the existing public Q4 selection contract.

Alias names introduce a separate observed behavior bug: native decoded names
Fn/Ctor/Literal print as `\u0046n`/`\u0043tor`/`\u004citeral` when those
are the first own declaration spellings. Public alias metadata will keep the
decoded name and exact origins. A private established Alias fact will carry
the canonical BoundSymbol, actual native alias arguments, provenance and
written display name. Only summary printing consumes the display spelling;
native cache keys consume canonical symbol/argument identities.

Source's alias recipe supplies the exact own declaration site and decoded
provenance. Evaluation resolves its canonical association through the existing
binder operation, then the complete symbol-written-name operation acquires
native own associations before creating a Model alias fact, preserving all typed
binding failures. Current nongeneric aliases establish absent type arguments;
generic aliases retain their existing explicit pending operation until the
actual native alias argument acquisition is implemented. No identity is
inferred from alias names, origins or the display string.

## Proof and remaining scope

Final public tests cover every admitted function/constructor signature family
and escaped aliases at summary-only, depth-zero and expanded depths. Private
retention assertions after final Project::load will pin both symbol roles,
their same syntax association, final node-symbol selection, exact reserved
partitions and unchanged lexical navigation. That private representation
proof is separate from public behavior and requires a full library checkpoint.
Purely retained role wiring has no invented public assertion RED; the actual
alias display bug has its own final assertion RED before its behavior changes.

The public two-test RED is
`vrun-fd09877e-f05d-4db4-87f5-362c5497d976`: both tests compiled and failed,
zero skipped. The alias test observed decoded `Fn` instead of native written
`\u0046n`; the signature test reached the explicit abstract-constructor
pending result after ordinary function, constructor and generic controls
passed. Production changed only after that receipt. It is not a passing
checkpoint; the final scoped GREEN and private library execution are still
required.

The next compiled public checkpoint was genuine RED
`vrun-bd8cdb8c-d7d4-4020-a6c1-8855967073cc`: the new merged-alias test
observed Alias instead of native `\u0041lias`; the original two tests exposed
missing FunctionType/ConstructorType FirstToken attachments in the shared
Source index. The index now captures these actual reserved declaration kinds.
No successful anonymous behavior checkpoint is claimed from that RED.

After the canonical role/name repairs, the final four-test public checkpoint
is GREEN in `vrun-dd868f09-e557-48bd-a55d-d22a6f897bc3`, zero skipped.
It covers the original function/constructor and alias families, complete
ordered merged-alias provenance, broader interface/function/namespace written
names, recursive function serialization and unchanged lexical reference groups
under strict/loose options and summary/depth modes. The two newly added private
role/group tests remain a separate library execution obligation until the
current full registered Rust checkpoint returns.

Fresh native controls expanded the rule across compatible value/type,
function/interface and namespace/type-alias meanings. The final five-test
native run is `vrun-6dd2cdd1-d52f-4f6b-a209-0866a6ca3f7a`, five passed and
zero skipped. Exact ordered own name spans for merged aliases were separately
GREEN in `vrun-6009ce97-336c-42bf-9928-2408e7cdd257`. A mistaken expectation
that an interface written before a function prints first was corrected from
fresh observations: Binder `bindEachStatementFunctionsFirst` visits functions
first in SourceFile, Block and ModuleBlock. Its whole two-pass iterator now
lives in syntax and both existing flow construction and binding collection
route through it; case/default statement lists retain ordinary source order.

The legacy lexical contract intentionally keeps nonmerging meanings in
separate groups. Native declaration associations therefore have their own
sparse Store representation only where those groups share an actual native
table entry. Bind appends associations at addDeclarationToSymbol; checker
merge preserves target-then-source order before any canonical module storage
swap. The representation adds no searchable name table and never reconstructs
order from Source scans or lexical origins. The common written-name operation
accepts BoundSymbol and acquires this native question internally, so aliases,
class/interface names, parameters and function serialization cannot supply a
caller-selected declaration slice. Alias provenance uses the same complete
association question and retains the decoded alias name.

The same Source owner follows FirstNonNil(GetNameOfDeclaration) across the
complete native association list, rather than requiring its first declaration
to be named. Actual reserved/signature type roles have no Name field and are
skipped. Named attachments retain their exact kind and token range; nameless
function/class declarations attach through actual first-token or modifier
sites without inventing a name. Native default-symbol fixed-context printing,
GetAssignedName, nameless-parent, late/nameType and internal-name fallback
branches need their distinct symbol/context facts. These valid unsupported
families return located UnsupportedSemantic failures, not Internal attachment
errors or guessed display names. The default branch is kept before selecting
that declaration's name. Current final controls cover ordinary named families;
these unadmitted fallback branches have no new behavior-completion claim.

The Alias cache key currently uses the stable TypeAlias representative of this
actual table identity. This bounded mapping does not establish universal
native BoundSymbol equality across every lexical meaning. Suppressed native
symbol replacement and a one-way copied meaning whose contextual identity the
legacy projection cannot retain remain explicit pending dependencies. Full
native symbol identity and those contextual merge factories remain required.
The added private association test pins final Project load across all five
line-break kinds, BOM and astral trivia, asserting the native combined own
associations and preserved separate lexical value/type groups. Its execution,
like the new Function/Constructor private role test, awaits a library checkpoint.

Capture uses a sparse transient owner-to-signature pair map, discarded after
the completed Store associations are published. It adds no field to every
retained declaration and no second searchable inventory. Function/constructor
nodes do retain two actual declaration associations and symbols, as the native
binder does. The private test asserts the allocation order, both native role
flags, final node-symbol lookup, own declarations, reserved partitions and
unchanged lexical alias references after final Project load across all five
line-break kinds, BOM and preceding astral trivia. The absent native parent
observation remains pinned compiler evidence; the Store has no native Parent
field, so that fact is not claimed as a separate retained runtime feature.

The admitted shared call helpers now use the same `fillMissingTypeArguments`,
`instantiateSignatureEx`, `getTypeParametersForMapper` and
`getTypeWithThisArgument` owners as references/member mapping. Call erasure
skips local clones; ordinary member mapping retains fresh local identities and
their actual mapper associations. Default slots currently use immutable mapper
snapshots. Native mutable-result-slice behavior needs its own complete protocol
before deferred anonymous default factories or inference callbacks are
admitted; current eager scalar/reference defaults do not establish that gate.

Native temporary-empty inventory publication during recursive anonymous
resolution, contextual/outer parameter acquisition, alias/single-signature
instantiation keys, late-bound members and representation/resource gates remain
required. This increment does not declare the full semantic scope complete.
