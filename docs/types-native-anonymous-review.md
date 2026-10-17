# Bounded anonymous symbol and alias review

This read-only Ousterhout review covers the current canonical
FunctionType/ConstructorType binder associations and Alias source/evaluator
boundary. It assumes one immutable accepted Project and request-local semantic
identities. It is not a full anonymous instantiation, generic-alias, or printer
review.

The retained role split follows pinned TypeScript 7.0.2
`binder.bindFunctionOrConstructorType` at binder.go:991. Native allocates a
Signature, associates the node, then allocates a TypeLiteral and overwrites the
main node.Symbol association; the former signature is held in the latter's
reserved member table. The implementation's transient owner/signature pair
records those two roles without inserting a lexical symbol. Link allocates the
Signature first, installs its own declaration, then creates the TypeLiteral
and attaches the real reserved member key. The main declaration lookup omits
only that intermediate FunctionType/ConstructorType role; signature lookup
still retains it. The pair map drops during retention. No runtime Symbol.Parent
fact is inferred from the member-table attachment.

The new final-Project private test asserts both distinct symbols, their native
flags, final node.Symbol, declaration lists, allocation order and lexical
compatibility under all five line break classes with BOM/astral offsets. At
this review point that new test is present but has not yet executed in a full
checkpoint. Older executed TypeLiteral retention proof does not prove this
new role increment.

Canonical alias binding is correctly fallible. `establish_alias` resolves the
actual alias declaration through the existing binding owner and preserves its
typed failure. Model stores canonical BoundSymbol and native argument identity
separately from public provenance and written display. A printer label is not
an intersection cache key. The current admitted nongeneric family proves
absent alias arguments; generic alias acquisition remains required work.

One concrete native written-name distinction needs a final reproduction and
fix. The current AliasRecipe captures the TypeAlias's own raw name, whereas
`getNameOfSymbolAsWritten` chooses the first named declaration of the canonical
symbol. Eight fresh strict/loose controls at
`.cc/temp/types-alias-written-review/answers.json` have empty config, program,
syntactic and bind inventories. With `const \u0041lias = 1; type Alias =
{ value: string };`, the actual alias symbol has Variable then TypeAlias own
declarations, and native summary is `\u0041lias`, while its decoded name is
Alias. Function-before-TypeAlias similarly prints `\u0046unctionAlias`.
Alias-first and unescaped-first controls retain the first alias spelling.
The final Project reproduction executed in
`vrun-bd8cdb8c-d7d4-4020-a6c1-8855967073cc` and confirmed the merged-name mismatch. The staged correction
uses the complete `Source::name_of_symbol_as_written(BoundSymbol)` operation;
it acquires `Bindings::native_declarations` internally. This is a different
question from the legacy reference cluster's declaration list. Its new native
five-test baseline passed in
`vrun-6dd2cdd1-d52f-4f6b-a209-0866a6ca3f7a`. The final four-test public Rust
rerun passed in `vrun-dd868f09-e557-48bd-a55d-d22a6f897bc3`, with zero skips.
That proves the admitted final summaries and graph outcomes; execution of the
new private role/group retention assertions remains the separate full
checkpoint obligation below.

## Sparse native declaration associations

The bounded peer read covered `Store`'s association groups, declaration append,
canonical union and transfer operations; Linker's `declare_in` and checker
`merge_entry`; the fallible `Bindings::native_declarations` facade; and the
written-name and alias provenance consumers. No additional blocking defect was
found in the staged ordinary named family.

Native `declareSymbolEx` (binder.go:154) appends compatible declarations to one
name-table symbol. The existing lexical reference contract deliberately keeps
some compatible meaning clusters apart. The sparse group stores their actual
ordered FileId/DeclId associations without adding another name lookup. A group
appears only when that distinction is encountered. Later
`addDeclarationToSymbol` appends at the existing binder operation, rather than
reconstructing order from positions, lexical sites, or a Source scan.

`mergeSymbol` (checker.go:14072) appends target declarations before source
declarations. The staged `Store::unite` shares the native group before the
storage-only module canonical swap, so that swap cannot reverse native order.
Both-way merges redirect the participating sparse associations and release the
old group's declaration payload. One-way copies retain a separate source
association where representable. A copied separate meaning whose contextual
symbol is not represented, or a suppressed native exclusion that replaces
node.Symbol, is marked unavailable and reaches semantic callers as typed
Pending. Neither becomes a guessed native declaration list.

The association map and ordered group payloads live with the retained Store.
They do not introduce a searchable declaration inventory or retain syntax.
Group headers and symbol handles remain after canonicalization; the read did
not measure aggregate bytes, worst-case merge work, or corpus-scale allocation.
Those are representation gates, not conclusions established by this source
read.

The shared `syntax::statement_bind_order` moves the whole native
`bindEachStatementFunctionsFirst` decision into one owner used by both Flow
and Collector. SourceFile, Block and ModuleBlock use the two passes; case and
default clauses retain ordinary statement order. The fresh controls show the
function's name can win even when an escaped interface name appears earlier in
the source. The final-Project private test separately pins native grouped
associations and unchanged lexical value/type reference groups under five line
break classes with BOM/astral offsets; execution of this new private test is
still a checkpoint obligation.

`Alias.symbol` remains the canonical TypeAlias representative used by the
existing type binding operation. Sharing a native declaration group does not
claim that every legacy BoundSymbol is a native SymbolId or that all grouped
meaning clusters can be interchangeable cache keys. Intersection alias caches
must use that established alias representative and actual arguments. The
written-name helper currently consumes admitted named declaration families;
native nameless/default/remapped/late-name branches remain distinct required
work. The final FirstNonNil delta uses the retained native syntax kind to
skip actual reserved nil-name roles and the existing Source attachment index
for named declarations. It does not turn an unavailable late/default/assigned
name question into a decoded-symbol fallback.
Native `GetNameOfDeclaration` (ast/utilities.go:1428) asks `GetAssignedName`
for function, arrow and class expressions after a missing own name. Therefore
an absent raw Name field does not establish the nil value that FirstNonNil
would skip. The staged early Pending for this unadmitted expression question
preserves that dependency; it is not the ordinary reserved-role nil branch.

Native abstract constructor printing is separate from alias printing. A lone
abstract ConstructorType uses the native Abstract signature flag and prefixes
`abstract new`; compound objects containing abstract constructors are
normalized to intersection syntax by the native node builder. The staged lone
constructor fix does not establish that compound family.

Full generic alias keys, object instantiation with alias arguments, native
late/computed names, compound abstract structures, resource boundaries and
corpus agreement remain mandatory gates.
