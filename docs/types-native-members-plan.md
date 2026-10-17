# Canonical own member binding and demand

The operating envelope is one immutable accepted TS/TSX Project generation,
constructed by the existing collector/linker and consumed by request-local
semantic evaluation. TypeScript 7.0.2 at commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b` defines the native decisions.
This is the next implementation increment, not a claim that Q3 or all member
type families are complete.

## Ownership decision

A source member-name map is inexpensive to introduce but cannot establish
native identity. A type parameter and a same-named property share one canonical
member symbol; paired accessors and method overloads combine declarations;
static and instance tables remain distinct. An independent semantic binder
would repeat scope/container, exclusion, merge and name decisions. The chosen
design extends the existing Decl arena, Store tables and canonical own
declaration associations. The lexical Index remains a projection: member names
keep their current NotReference role, with no second searchable token array or
new lexical reference groups.

`bindPropertyWorker`, `bindPropertyOrMethodOrAccessor`, `bindParameter`,
`declareClassMember` and `declareSymbolAndAddToSymbolTable` attach typed member
declaration roles to their actual captured declaring container. Collector keeps
the actual role and raw own anchor, including separate parameter and parameter
property roles at the same anchor. The existing iterative traversal context
must distinguish native declaration container from lexical scope; nested type
and object literals cannot accidentally bind to the enclosing class/interface.

`getDeclarationName` owns early literal/private/reserved/dynamic keys.
`getPropertyNameForPropertyNameNode` is a related, distinct syntax operation:
notably early binder `+1` and the property-name query's `1` are different native
questions. Parsed literal payloads stay exact. Unpaired UTF-16 identity remains
an explicit failure at its existing admission/demand boundary. Private names
carry the actual containing class identity; internal signature names occupy a
typed key domain and cannot collide with user strings. Dynamic names retain
their own anonymous symbol/declaration for later binding; they do not silently
become an empty named inventory.

Store continues to own one canonical SymbolId and the native declaration order.
Its existing public string-name entries may carry an actual member symbol in
addition to lexical meaning slots; private and reserved keys use sparse typed
partitions of the same table. These partitions distinguish native key domains,
not competing binding decisions. No insertion-order copy of string names is
needed: native `getNamedMembers` partitions own/inherited members and
`compareSymbols` sorts each partition by first actual declaration's program
file index/source position, then exact symbol name and native identity ties.

The native member `declareSymbol` question always combines compatible roles
at one key, including property/type-parameter meanings. It must not reuse the
existing lexical separation rule simply because both use a table. Exclusions
extend the existing flags owner with Method/GetAccessor/SetAccessor/Signature/
Constructor/Optional/Prototype distinctions. Reached reported bind conflicts
still fail Project loading; a suppressed conflict retains the native separate
declaration symbol without overwriting the first table entry. Existing lexical
compatibility behavior remains its separate established contract.

Type-parameter lookup at class/interface lexical scope must still ask native
TypeParameter flags, rather than expose ordinary member values as unqualified
lexical variables. Parameter declared-type/constraint/default operations filter
the canonical symbol's actual TypeParameter associations instead of requiring
all own declarations to have that role.

## Consumer boundary

Binding exposes complete opaque-symbol operations:

- `member_of_declaration(file, raw_anchor, role)` obtains the actual canonical
  member role; parameter/property pairs remain distinct.
- `declared_members(owner, instance_or_static)` returns canonical early member
  symbols, reserved signature families and any pending late-member obligation.
- `member_declarations(symbol)` returns actual typed own declarations in native
  bind/merge order. Public locations are formatting, never identity.

Source projects a demanded canonical member's type/signature/accessor recipe
from those actual attachments. Evaluation caches ordinary member reads by the
canonical symbol and instantiated reads by original native member plus mapper;
the current source-origin MemberRoot registry remains only for its separately
admitted anonymous single-property family until real anonymous container binding
replaces it. Named class/interface member selection and inventory must migrate
to this canonical operation together, not retain a source-spelling second rule.

The first final public proof selects both raw declarations of a merged property
and pins first-value-declaration type precedence, then expands a merged generic
interface's actual canonical inventory. Methods, paired read/write accessors,
auto-accessors, parameter properties, private/static keys, real computed late
members, class prototypes/constructors, inheritance and full signature mapper
substitution remain required subsequent producer/consumer work. Until each
family's complete operation is admitted, its demanded recipe stays located
Unsupported rather than a fabricated known-empty table or healthy Any.

## Evidence and validation sequence

Fresh native controls in `fixtures/types/native-member-tables/members.ts` and
`tools/type-member-tables-oracle.test.mjs` pass five registered tests with zero
skips: `vrun-9061262a-2bd0-49e4-823b-8b03e7d02a0d`. They pin shared
TypeParameter/property identity, native ordered associations, merged property,
method overload and paired accessor identities; static/instance separation;
private native names; auto-accessor flags; exact empty/dotted/numeric/computed/
escaped keys; and actual first-declaration member order. Four input diagnostic
inventories are empty and there is no semantic-diagnostic checking prelude.
These compiler facts do not prove product retention or Q3 search relationships.

The first two final public tests turned GREEN in
`vrun-f564dcd9-dd38-47d5-b546-33592eeb8046` after an actual assertion RED.
Expanded own-origin coverage exposed a missing complete association list:
`vrun-bea1e0c7-98cf-496f-b58b-104c0b96ebe7` had three passes and one failure,
then four final tests passed in
`vrun-4a549888-b1e5-4b09-84bd-98d6f7a6d15f`. A fresh computed-name selection
control exposed operand interception in
`vrun-d86c289c-d8fe-4e2e-b98b-2b958cce821e` (four passes, one failure).
Restricting actual declaration-name selection to its direct native name-token
kinds preserved the shared expression-context owner. Five final tests now pass
in `vrun-637db28d-2302-4674-a414-1952fa213b27`, zero skips. These pin actual
merged first-value precedence, exact literal/escaped keys, canonical inventory
order, complete own origins, separate shared-symbol value/declared questions,
and the computed literal operand's Expression question.

The independent Ousterhout review in `types-native-members-review.md` found no
other incorrect answer in the admitted ordinary property family. It identified
repeated full captured-AST acquisition: one scan per inventory member and a
further scan per demanded read. A single exact final-site attachment owner is
planned to remove that O(member count × AST size) cost; no measured resource
violation or completed performance gate is claimed. Retained Entry/member key,
value-declaration and late-member payload costs still need actual accounting.

Run the final public file through the registered single-file TDD route before
production. Mechanical fact/DTO wiring earns no artificial RED; its meaningful
retention assertions must execute after final Project::load in a private library
checkpoint, separately from public outcomes. Repeat established navigation,
lexical references, generic/Array/JSON and whole-load checks at a coherent source
checkpoint. Record actual retained table/association cost and latency before the
full semantic resource gate closes. A source read or native-only suite does not
substitute for those product proofs.
