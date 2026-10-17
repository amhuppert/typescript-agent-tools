# Exact native literal values

This records the design review and ongoing implementation. Source edits alone
do not complete a literal factory or consumer; final-query validation remains
required. The operating envelope is the
immutable, TypeScript-only Project and its request-local evaluator. Native
behavior is TypeScript Go 7.0.2 at
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`.

## Recommendation

Keep one `Type::Literal` / `TypeView::Literal`, with its existing flags,
fresh/regular pair, enum association and factory. Replace the implicit
`Arc<str>` payload contract with a private, canonical `LiteralValue` owner.
Ordinary scalar text remains the common representation. String values can also
retain exact native byte runs and opaque symbol-identity components. There
must be no implicit `Display`, `Deref<Target = str>`, lossy conversion, or
placeholder spelling for that representation.

The initial factory move can support identity/flag-only paths and leave an
explicit failure at a reached text operation whose semantics are not yet
ported. That is a coherent incremental boundary, not completion of the full
literal family. In particular, the private `unionMissing` path must remain a
known failing final test until the actual factory and its consumed operations
are implemented and measured.

There are two policy prerequisites for complete support: splitting an opaque
identity during template inference, and exposing such a value in the public
literal graph. The existing [identity policy](types-design.md#evaluation-order-baseline)
settles identity, ordering and approximate length; it does not settle either
of these operations. Do not infer a new exception from the existing one.

## Why the current boundary is real

`property_names.rs::string_literal_type_for_property_key` currently rejects
generated keys and non-UTF-8 fixed keys. The native question at
`checker.go:getApplicableIndexInfoForName` (18921) is
`getStringLiteralType(name)`, whose cache at 25213 accepts an exact Go string.
The factory itself has no UTF-8 eligibility test.

Its input is not necessarily a user-written string. `NativePropertyKey`
already represents fixed bytes, private class identity plus description, and
unique-symbol identity plus its underlying name. Unique names are separately
routed to `symbolType` by this particular applicability caller. That shortcut
does not make all other literal consumers UTF-8-only.

Three different native questions must remain separate:

* `getPropertyNameFromType` (`utilities.go:863`) returns the literal's exact
  value as a table key. It cannot use printed text.
* `getLiteralTypeFromProperty` (`checker.go:26617`) can ask `ast.SymbolName`.
  That operation returns the actual private declaration name when there is a
  private ValueDeclaration (`ast/symbol.go:72`). Raw `Symbol.Name` does not have
  that substitution. Do not put the substitution in the literal factory.
* `EscapeString` (`printer/utilities.go:77`) intentionally escapes a stray
  invalid UTF-8 byte as `\uFFFD` in printed output. This output operation does
  not authorize replacing the stored byte or merging its identity with an
  actual U+FFFD string.

## Alternatives and closure

An extra `NativeStringLiteral` type variant makes the first factory small, but
duplicates a native type category. Every STRING_LITERAL/UNIT/FRESHABLE rule,
literal widening, equality, enum handling and exhaustive `TypeView` consumer
would have to remember both variants. Existing matches that accept only
`Literal` could silently take a nonliteral fallback. The variant would hide
the change from some callers that need compile-time pressure.

A unified `LiteralValue::{Text, NativeKey}` is a better first implementation
boundary. It preserves the existing type category, requires payload readers
to confront the new value, and can reuse `NativePropertyKey` identity directly.
It is not the final closed string domain: native templates concatenate a key
with arbitrary text, and string mappings can modify a generated prefix.

The complete internal string value therefore needs a canonical sequence of
exact bytes and opaque identity components. For example, a private name has
the logical components `FE#`, its actual class identity, its native separator,
and its exact description. A template may add bytes before or after those
components. Adjacent bytes coalesce; empty byte runs disappear; an atom is
never reconstructed from user text or rendered numeric IDs. An ordinary UTF-8
byte-only sequence canonicalizes to the same `Text` value as a source literal.

This should be an immutable flat value, not a rope, expression interpreter,
global registry, or pluggable string backend. The current envelope does not
need those mechanisms. A compact `NativeKey` storage form can remain an
internal optimization behind this value owner, provided equality, hashing and
normalization have only one definition.

Do not build a parallel string identity that disagrees with property keys.
Conversion from a generated key must preserve all recursive components, and
`getPropertyNameFromType` must recover an equivalent key. When transformed
strings become remapped property names, the key owner must accept the same
general components or the reached conversion must fail explicitly. A
`NativePropertyKey::Compound` added without canonical equality against the
existing Private/Unique forms would create duplicate identities.

The current flow-cache byte/identity builder demonstrates adjacent-byte
coalescing, but its node/type identity components and write-time observation
rules answer another native question. Do not make literal creation observe
flow ordering or share that stateful builder just because the shapes resemble
one another.

## Value-owner contract

The model alone constructs payloads. A native string payload is valid only for
the String base; number, Boolean and BigInt canonical text retains its current
scalar contract. Enum caches still distinguish enum identity, and fresh
literal allocation copies the payload and enum/symbol association unchanged.

The canonical string factory joins the existing regular-literal interner.
Equal UTF-8 text from a fixed key and a source literal must return the same
regular TypeId. Equal private descriptions from different class identities
must remain unequal. Interner lookup and equality do not observe identity
order. No additional fresh allocation occurs until `getFreshTypeOfLiteralType`.

Payload operations should expose the native question, not an optional string
that callers can interpret as false. Structural identity, emptiness, copying
and base flags are total. Reached semantic string equality is fallible: unequal
component sequences may still spell the same bytes, and deciding that can
require an opaque identity's digits. Native-name roundtrip and comparison
preserve the structured identity. Operations requiring concrete text either implement the
whole native operation or return an explicit unsupported dependency from its
existing evaluator owner. `None` is reserved for an actual native absence or
failed match; it must not mean that the representation was unavailable.

`CompareTypes` uses raw string comparison (`checker/utilities.go:490`).
`LiteralValue::compare_value` owns that fallible byte observation, separately
from `IdentityOrder`'s adopted property-key ordering. Fixed prefixes decide
immediately; equal whole atoms advance without revealing their widths. End is
less than any nonempty remainder. At an atom versus a fixed byte, the native
positive-decimal first-byte range `1` through `9` can prove order outside that
range. Otherwise the operation returns `IdentitySpelling`, which becomes a
located `UnrepresentableIdentity` when consumed. It never substitutes ranks or
type IDs. Recognized Private/Unique shapes receive no literal-order exception.
Length continues the existing exact-bytes-plus-one-per-atom display rule and
never allocates an ordering rank.

## Complete consumer migration inventory

Line numbers are review anchors, not permanent interfaces. Re-run the
`Type::Literal` / `TypeView::Literal` search at implementation time, and also
find direct literal factory/value helpers and `TemplateLiteralView.texts`.

| Owner | Required migration |
| --- | --- |
| `model.rs` (1874, 2731–2857, 3257–3275, 3564, 3625, 4069) | Type/View payload; regular and enum interner keys; fresh copies; value equality including numeric NaN; literal sort; fixed-string comparison; flags and model tests. Preserve one native literal category. |
| `model/templates.rs` | Exact template text segments and cache key must accommodate results of native concatenation. StringMappingKind must call the shared string operation instead of assuming `&str`. |
| `property_names.rs` (306, 696, 842) | Exact key-to-literal and literal-to-key roundtrip; applicability order unchanged; numeric-name classification uses its actual string question. FE/private values are not numeric canonical spellings, but inability to represent a value is never a general relation-false answer. |
| `facts.rs` (161) | String emptiness is independent of UTF-8; numeric/Boolean truthiness still reads its scalar payload. |
| `expression.rs` (469), `operators.rs` (575), `property_symbols.rs` (1631) | Preserve numeric/Boolean literal operations and their base guards. Do not introduce text admission into branches that only need those scalar values. |
| `templates.rs` (199, 377) | Whole template flattening/concatenation and string mapping. Preserve cross-product and placeholder acquisition before reached text work, and preserve existing cache publication. |
| `templates/matching.rs` (37, 175, 242, 254) | Prefix/suffix tests, byte delimiter searches, native rune advancement, captured slices and placeholder validation. Representational failure must not become no match. |
| `inference/templates.rs` (34, 137, 162, 185, 207) | String/numeric/BigInt/Boolean placeholder inference and exact source value tests; retain ordered constraint/relation callbacks. |
| `relations/reporting.rs` (879, 889) | Spelling suggestions operate on the actual literal value; no suggestion or equality decision from a display name. |
| `native_display.rs` (434, 1086), `native_display/symbols.rs` (57, 89) | Literal printer and nameType classification; exact native length precedes printing. Distinguish literal value printing from private declaration-name printing. |
| `presentation.rs` (469, 1915), `types.rs::TypeNodeData::Literal` | Q4 summary admission stays lazy; summary escaping and expanded graph value are different contracts. A graph String field cannot losslessly carry bytes/identity atoms. |
| Payload-free matches in `intersections.rs`, `unions.rs`, `structure.rs`, `generics.rs`, `property_symbols.rs`, `constraints.rs`, presentation kind/edge enumeration | Keep ordinary literal flags/category behavior. These branches should not need a text getter merely because the payload can now be native. |
| `binding/ordering.rs` plus all key conversions | Retain one key identity/order and generated-prefix classification. No source spelling, origin list or ordering rank becomes the internal value. |

The Source pseudo-literal descriptors are syntax nodes, not semantic literal
payloads. Their `PseudoType::Literal` matches do not require a second value
representation; semantic realization and existing-node printing must continue
through their actual owners.

## Native text semantics and unresolved policy

`getTemplateLiteralType` (`checker.go:28901–28976`) writes the literal value
into a byte string, joins exact text spans, and calls `CombineSurrogatePairs`.
That string utility preserves unrelated invalid bytes. The current Rust
`encode_utf16` flattening cannot be used for native raw byte values: it cannot
represent FE and would erase the distinction before interning.

`applyStringMapping` (28999) and `stringutil/js_case.go` decode native JS runes.
Unlike concatenation, casing intentionally converts a stray FE byte to U+FFFD
when it writes the decoded rune. This is a semantic transformation, not an
admission decoder. Capitalize/Uncapitalize transform only the first decoded
rune and append the remaining original bytes. Lowercase also has contextual
final-sigma behavior; independently mapping arbitrary byte chunks would lose
context. Decimal identity text consists of uncased, non-case-ignorable digits,
so its opaque component can preserve that boundary fact without choosing the
digits, but this extension still needs measured controls.

`inferFromLiteralPartsToTemplateLiteral` (`relater.go:2385–2470`) searches byte
delimiters and, for an empty delimiter, advances one native JS rune. It can
therefore split the decimal spelling of a compiler ID. Treating an opaque
identity as one indivisible character is an additional semantic exception;
the existing approximate-length rule does not establish it. A representation
can preserve a whole captured atom without issue, but a reached operation that
needs to inspect or split its hidden spelling must remain explicit until that
policy is decided. It cannot return a false match, empty capture or primitive
String as a substitute.

Raw-byte printing can be implemented exactly in the shared printer: decode
with native JS-string rules and escape stray bytes as native does, leaving the
stored value unchanged. A generated atom has no permitted decimal spelling.
Printing `#description`, an origin path, `<private>`, or a request rank would
invent a value. Similarly, `TypeNodeData::Literal.value: String` is a decoded
value contract, not a quoted-syntax field. Full graph support requires an
explicit structured value extension or a documented product representation;
until then, a reached graph-value acquisition must remain a failure. Diagnostic
quoting must not be reused to silently change that field's meaning.

Lone-surrogate source strings remain governed by the parser's existing input
envelope. The exact-byte owner can preserve native surrogate sentinel bytes,
but its introduction is not authorization to broaden accepted source syntax.

## Implementation and proof sequence

1. Introduce the unified payload privately and migrate every payload reader.
   Keep existing source-string factories convenient. Canonicalize valid fixed
   UTF-8 into their existing identities; preserve raw keys without allocating
   fresh types or identity ranks. Route all flag-only consumers unchanged.
2. Port exact equality, native-key roundtrip, truthiness, fixed-string tests,
   applicable index behavior and ordering through the existing owners. At any
   remaining reached text operation retain a named, located dependency.
3. Port raw-byte printer and concrete byte text operations as whole native
   owners. Extend template text storage only with the same canonical value
   contract. Resolve opaque splitting/order/output policy before admitting
   those operations for atoms; do not scatter per-caller approximations.
4. Measure native behavior, then assert final Project outcomes. Preserve
   existing private, unique, enum, template, relation and diagnostic regressions.
   A factory unit test does not establish the final behavior.

The first final-query matrix should include the existing private unionMissing
case, same-description private members from distinct classes, inherited and
shadowed private owners, missing-private versus ordinary missing keys, and
ordinary/unique controls. The native observer should record only an already
acquired key literal and factory-hit identity at the actual call; it must not
request another literal merely to inspect it.

Add string, number, symbol and template index signatures around those source
paths, including template prefix/suffix, empty delimiter, two inferred
placeholders, and a later unresolved annotation after the key operation.
Those cases distinguish successful flags-only applicability from an actual
text dependency and verify continuation/error order. Do not assume every
internal name is reachable through every source construct: first establish
the actual native acquisition, and use same-checker native factory controls
only as supporting evidence when source reachability is absent.

For exact value behavior cover empty/ASCII/Unicode fixed keys, FE and actual
U+FFFD as distinct stored values, different class atoms with equal descriptions,
repeated same-key acquisition, all four casing operations, and concatenation
on both sides. Ordering controls must retain the adopted FE#/FE@/fixed-family
rules without serializing native IDs. Public graph/summary controls must test
their separate admission boundaries. The source reachability capture below
covers part of this matrix; the remaining controls are still unmeasured.

## First measured source reachability

The parent ran the frozen
[`native-generated-literal-keys`](../fixtures/types/native-generated-literal-keys/cases.ts)
fixture through the fresh API and the same-checker hook. Run
`vrun-f5cb7d82-c82d-4165-adea-db52ce044762`, receipt
`.cc/temp/stage36-generated-literal-native-receipt.json`, completed all 12 roots
in both strict and loose profiles with matching summary/errorType. Both test
groups stopped only at missing expected files, after writing all captured
rows; this is bootstrap evidence, not a maintained green baseline.

Every root accesses the actual private `#field1` on a union of its declaring
class and one other constituent. Both profiles produced:

| Other constituent / case | Native result | Native diagnostic codes |
| --- | --- | --- |
| Empty object (`absent`) | errorType `any` | 2339 |
| String index | healthy `string \| number` | none |
| Number index | errorType `any` | 2339 |
| Symbol index | errorType `any` | 2339 |
| Index matching `${string}#field1` | healthy `string \| number` | none |
| Index matching `${string}#${number}@#field1` | healthy `string \| number` | none |
| Same prefix with actual U+FFFD instead of raw FE | errorType `any` | 2339 |
| Index matching `${string}${number}` | errorType `any` | 2339 |
| Wrong fixed suffix | errorType `any` | 2339 |
| String index with `MissingStringIndex` value | errorType `any` | 2304 |
| Number index with `MissingNumberIndex` value | errorType `any` | 2304, 2339 |
| Matching suffix index with `MissingTemplateIndex` value | errorType `any` | 2304 |

The successful ID-spanning template is concrete evidence that supported
private access reaches template capture and numeric-placeholder validation.
The whole opaque identity component denotes decimal native symbol-ID text;
retaining that lexical property may suffice for this validation without
materializing an ID. It does not establish the value of an inferred numeric
literal, the first digit of the atom, or atom-splitting semantics. The suffix
and replacement-prefix twins also rule out replacing raw FE with U+FFFD before
matching. A flags-only first factory must leave these healthy template cases
as explicit unfinished obligations, not classify them as nonmatches.

The captured diagnostic arrays are source-ordered history, not acquisition
order. The existing acquisition records show native getter/type-node
activity, but do not directly observe each template callback. The differing
final results together with `getApplicableIndexInfoForName` and template
matching source establish these paths; the array order alone establishes no
new cache or recovery order.

Next probes should try a mapped index constraint with conditional/inferred
template parts, a contextual index path, and a `keyof`/mapped-name route, each
with a healthy twin and a later unresolved dependency. Where such a route
reaches the private raw name, distinguish whole-atom capture from an empty
delimiter that steps into the decimal spelling. Separately try all four
intrinsic string mappings and expanded graph/diagnostic display of a captured
literal. These are reachability experiments: unsupported source constructs or
paths that never acquire the key do not prove that the underlying operation
is safe to omit. Any observer added for them must read already-acquired
arguments/results and must not invoke a factory, matcher or mapper itself.

## Context and decimal-width evidence

The first key matrix passed maintained validation in
`vrun-dc9a1af6-d56f-4c34-a877-5260660fc34d`: 24 queries, two groups, zero skips.
The contextual/width matrix in `fixtures/types/native-generated-literal-context`
passed maintained validation in `vrun-8d8efe90-e60b-4649-99a2-601aea1cf883`: 36
queries, six groups, zero skips. Bootstrap capture
`vrun-3679e9b5-b958-4981-b098-fc774d814194` had API/hook parity and only six
missing-baseline errors before those exact rows were installed.

Contextual generic mapped types expose the private static member's raw key as
an arrow parameter literal. Whole-ID extraction returns its decimal string,
concatenation preserves FE, and all four intrinsic mappings intentionally turn
the first invalid FE byte into an actual U+FFFD rune. Public static, instance
private, keyof and mapped-name controls take their different native branches.
The private instance parameter is ordinary Any, with diagnostic 7006 only in
the strict profile; that diagnostic does not create native errorType.

The index template `${string}#${number}${number}@#field1` rejects the cold
one-digit owner and accepts the owner preceded by nine private-class
declarations. Whole-ID and suffix controls accept both histories. This proves
that generated-digit splitting is reachable and depends on process allocation
history. The design now explicitly refuses digit-dependent observations with
UnrepresentableIdentity; it does not turn an unavailable split into a failed
match. Whole-atom operations and exact fixed-byte operations remain required.

For output, retain ordinary literal JSON unchanged and add a distinct symbolic
encoding rather than making its existing `value` ambiguous. The proposed
encoding contains exact text/byte parts and document-local symbol references;
metadata can supply declaration origins without merging identities that share
origins. A visibly algebraic summary must distinguish these references from
actual TypeScript string text. Concrete wire changes, budgets and final-query
proof are still pending; the native captures do not establish their Rust
implementation.

## Exact template and casing migration

The next implementation boundary is one `LiteralValue` domain for both literal
payloads and stored template text spans. The operating envelope remains an
immutable Project with request-local factories. Keeping scalar template spans
and adding only a native concatenation branch would require another algorithm
for matching, mapping, sorting and output. Migrating the existing span domain
removes that dependency without adding a second type category or string engine.
Production migration is staged after its behavior-RED checkpoint; Rust GREEN
and the follow-up observations listed below remain pending.

### Operation ownership

`src/strings.rs` owns the native byte operations `EncodeJSStringRune`,
`DecodeJSStringRune`, `CombineSurrogatePairs`, `ToUpperJS`, `ToLowerJS`, and
`isFinalSigmaContext`. Extend its existing decoder and pinned casing tables;
replace the scalar-only case loops rather than retaining another implementation.
Casing copies lone-surrogate sentinel bytes and intentionally writes U+FFFD for
other invalid bytes. `CombineSurrogatePairs` combines adjacent high/low sentinel
pairs and copies every unrelated byte, including FE, unchanged.

The canonical value owner keeps `from_parts` and ordinary `concat`
representational: adjacent bytes coalesce and opaque identities are retained.
Native concatenation normalization is a separate value operation routed through
`CombineSurrogatePairs`; the template factory applies it at each native
`addSpans` boundary and matcher `addMatch` applies it to single-span captures.
The value can convert source UTF-16 fragments into exact native bytes without
losing lone halves. Source recipes can keep their existing UTF-16 representation
until the evaluator's input boundary; source admission need not be redesigned.

`StringMappingKind::apply` takes and returns `LiteralValue`. Upper/lower map each
canonical byte run through the single byte-casing owner and retain each atom.
This is sound because canonical runs are separated only by positive-decimal
identity atoms: every possible digit is uncased and non-Case_Ignorable, so it
resets Final_Sigma context on both sides. Independently mapping arbitrary byte
chunks is unsound. Capitalize/uncapitalize transform the first decoded rune,
then append the original remainder unchanged; an initial identity atom is
unchanged without observing its first digit.

### Construction and consumer migration

1. Change `TemplateLiteralView.texts`, `Model::template_literal`, and its existing
   cache key to `Arc<[LiteralValue]>`. Adapt the startup numeric-string template.
   Keep a single `Type::TemplateLiteral` and interner.
2. Make `template_literal_type_completion` the single exact-value implementation
   of `getTemplateLiteralType`. Convert source UTF-16 fragments at their existing
   entry points; remove semantic literal roundtrips through scalar text/UTF-16.
   Preserve union distribution, cross-product checks, wildcard handling, ordered
   `addSpans`, invalid-placeholder widening, and cache publication. Accumulate a
   local vector of values and concatenate once per surviving span to avoid
   repeatedly copying the growing prefix. Return `regular_string_literal_value`
   when no placeholders survive. `getTemplateStringForType` keeps canonical
   number/Boolean/BigInt and nullable spellings without a string admission gate.
3. Route `getStringMappingType` and `applyTemplateStringMapping` through exact
   value mapping. Preserve the existing order: all text spans before mapped
   placeholders for upper/lower; only the first nonempty span or first
   placeholder for capitalize/uncapitalize.
4. Extend `templates/matching.rs` to exact target spans as well as exact source
   spans. Remove source conversion and crossed-capture scalar projection.
   Equal whole atoms can advance together. A nondigit proves a mismatch against
   an atom; digit-dependent prefix/suffix/search decisions and slices fail with
   `UnrepresentableIdentity`. Different atoms in substring comparisons can still
   require their decimal spelling. `templateLiteralTypesDefinitelyUnrelated`
   becomes fallible and receives the relation consumer location.
5. `inference/templates.rs::inferToTemplateLiteralType` applies mapping constraints
   to the exact captured value while preserving constraint selection order.
   `generics.rs` and `constraints.rs` pass exact spans into the same factory.
6. `model.rs::CompareTypes` uses `LiteralValue::compare_value` for string values
   and template spans. This native byte observation is separate from the
   established generated property-key policy. No derived atom ordering or new
   rank observation is introduced; ordering that needs actual digits remains
   an explicit `UnrepresentableIdentity`.
7. `relations.rs` propagates fallible unrelatedness and compares template spans
   with the appropriate semantic equality operation. Structural interner
   equality remains useful for the identical-span fast path, but an unequal
   cache key is not evidence for a digit-dependent semantic mismatch.
8. Output consumers in `native_display.rs`, `native_display/syntax.rs`, and
   `presentation.rs` carry fixed template bytes to the existing `quoted_bytes`
   printer owner. Synthetic template nodes and reused source tokens remain
   distinct. Preserve placeholder acquisition and bounded-output schedules.
   Opaque output keeps the explicit symbolic-projection dependency until the
   separate public-output contract is implemented.

### Equality is a reached semantic observation

An extracted `Identity<P>` can be compared to a concrete decimal string by a
conditional type; it is no longer protected by the raw FE prefix of the full
private name. `NativeStringValue`'s derived equality is the canonical interner
identity operation. It must not answer whether an atom's hidden decimal spelling
is `"1"`. The native string-equality owner must preserve three outcomes: proven
equal, proven unequal, and `IdentitySpelling` when the answer needs hidden
digits. Equal component sequences prove equality, while a concrete nondigit can
prove inequality against an atom. Keep structural interner hashing and equality
unchanged, and route reached semantic string comparisons through the fallible
operation.

The staged inference constraint path routes `template_constraint_literal_equal`
through the shared semantic operation. General conditional relations use it in
`raw_type_related_completion` and the active relation session: `source == target`
proves equality, but differing literal TypeIds must not silently decide the
atom-versus-digits case. `Model::literal_value_equal` is now fallible and its
existing enum comparison callers propagate that result; changing that helper
alone would not have covered the reachable conditional path.

The context checkpoint `vrun-a25d0d9f` reached a separate prerequisite: after
`IdentityPart<P>` captures the whole atom, `getConditionalType` in
`conditional.rs` instantiates its extends operand with the inferred mapper.
`instantiateTypeWorker(TemplateLiteral)` then reaches the scalar projection in
the existing template factory. The matcher did not split the atom; rebuilding
the inferred target requested its digits. Exact template spans remove this
projection and let source and target advance over the same whole atom. This
also explains why testing only the matcher cannot prove whole-atom capture.

### Final-query evidence to add

`fixtures/types/native-generated-literal-operations` stages 28 queries per
strict/loose profile. They consume generated values and return ordinary scalar
answers, isolating operation behavior from the unfinished symbolic-output
format. The matrix covers raw FE versus U+FFFD, concatenation, all four mappings,
raw bytes after a mapped first rune, expansion mappings, Final_Sigma beside an
atom, surrogate pairs across joined fragments, lone/reversed sentinels, and
retained template-placeholder capture/mapping. Native API/hook captures must be
installed only from the registered harness run, then replayed successfully.

The corrected native capture `vrun-4a60df15-4691-460b-b80a-b92ac87fc286`
produced all 56 healthy rows and failed only because expected files had not yet
been installed. Maintained replay
`vrun-6cea59b6-79cf-42e7-b96c-1cae52586798` passed both strict/loose profiles,
with API/hook parity, no error types, no diagnostics, no demand failures, and
zero skipped tests. The first bootstrap was discarded because fixture aliases
named `Capitalize` and `Uncapitalize` shadowed the intrinsic globals; all local
operation aliases now use an `Operation` prefix.

Rust behavior RED `vrun-1e95dfe8` reaches the current scalar projection at
`concatTail` and fails the healthy `sameIdentity` control after the expected
hidden-digit failure. The unsupported test already asserts the exact contextual
parameter location. These failures establish the prerequisites described above;
they are not a post-migration pass.

The equality controls distinguish the same atom and an impossible nondigit
spelling from concrete decimal equality. `identityConcreteDigits` records native
reachability; its product expectation is `UnrepresentableIdentity`, independent
of the captured process ID. Existing cold/warm split controls retain that same
explicit policy. Native summaries containing process digits are never copied
as product expectations.

The operation migration is complete only after final Rust queries fail before
the fix and pass afterward, together with existing template normalization,
matching, mapping, expression, index and product-limit checks. The maintained
36-case native context evidence proves reachability, not a Rust pass or completion
of symbolic output or composed-value ordering.

### Staged ownership and remaining observations

The staged implementation replaces scalar casing loops in `src/strings.rs`,
stores exact `LiteralValue` spans in the existing template model, and removes
semantic template roundtrips through UTF-16. `LiteralValue` owns prefix, suffix,
index, rune advancement and fallible equality; matcher captures and inferred
target reconstruction use the same factory. `TemplateLiteralView` owns the
fallible `slices.Equal(texts)` decision shared by inference and identity
relations. Plain literal relations, including the active identity branch and
nullable-union normalization, and existing enum literal comparisons use that
same value equality. Interner keys retain structural equality.

Bounded read-only reviews by the root and the literal/facade review lanes found
no confirmed defect in the staged byte casing, surrogate normalization, exact
matching, or equality cancellation rules against pinned native source. In
particular, digits separating canonical byte runs reset Final_Sigma context;
equal whole atoms may cancel; distinct isolated atoms prove inequality; and
adjacent unequal atom sequences remain spelling-dependent. This is a source
review disposition, not a compiler or final-query GREEN claim. The root owns
the frozen combined Rust validation. The generated operations target passed in
`vrun-21eaf6bb-e2dd-4bf3-823e-a475816b9e31`; the generated context target passed in
`vrun-47cd94bb-9a20-4a53-b069-70865c034ac6`. Those receipts prove the staged
operation/context matrices, not closure of the remaining observations below.

Native `relater.go::inferFromLiteralPartsToTemplateLiteral` compares fixed
prefixes and suffixes before searching delimiters in source order. The value
walk stops at the first decisive byte or whole equal atom; a nondigit rejects
an atom, while a digit-dependent prefix/search requires spelling. Structural
positions establish overlap after successful prefix/suffix walks without
inventing decimal widths. `checker.go::getTemplateLiteralType` normalizes with
`CombineSurrogatePairs` when `addSpans` retains a placeholder and after the
final span; `addMatch` also normalizes a single captured span. Concatenating
canonical components alone performs no Unicode normalization.

The scope does not close every native-string consumer. Ordinary source literal
admission and enum/diagnostic literal construction still contain UTF-16 scalar
projection gates. Composed property-key lookup and ordering remain separate
observations; no numeric symbol rank was introduced. Abstract template spans
with opaque atoms still depend on the separate output projection contract.
The current final-query matrix covers same-atom equality, impossible nondigit
equality and concrete-digit dependency, but dedicated final-query controls for
distinct captured atoms and adjacent-atom partition equality remain to add.
The separate `native-generated-literal-equality` fixture is being staged by the
literal-model lane for those controls. They require independent contextual
class owners; structural equality
must not classify differently partitioned decimal concatenations as unequal.

### Reached anonymous-class declaration identity

The generated-output RED in
`.cc/temp/stage39-type_generated_literal_output-output.txt` reaches
`outer_type_parameters` while instantiating the nested tuple type in
`distinctTuple` and the hidden-summary control. The `This` source recipe had
reduced the enclosing anonymous class to `declaration_anchor`, its first
`class` token. `binding/collect.rs::class` retains that anonymous class's own
header at the full node span; an anchor lookup therefore reported NotRetained.

Native `checker.go::getOuterTypeParameters` at 23664 uses
`getSymbolOfDeclaration(node)` before asking for the class/interface's this
type. The staged two-file fix carries `DeclarationIdentity` in the `This`
recipe and resolves it through existing `Source::symbol_of_declaration`, whose
binding call compares actual kind and node span. This applies equally to named
and anonymous class frames and interface frames, retaining the existing frame
order and lazy acquisition. No binding fallback, synthetic symbol, name-token
reconstruction, or change to this-parameter deduplication is introduced.
The operating assumption is the existing retained canonical syntax tree;
post-fix output validation remains pending.

The next output/private/equality RED in
`.cc/temp/stage40-type_generated_literal_output-output.txt` advanced to the
obsolete `Source::declared_target` preflight. That recipe attempted a second
class declaration selection from name anchors and rejected the anonymous
class before publishing its target. Its only consumers were the already-known
class/interface kind and diagnostic provenance. The staged cleanup removes
that recipe: `class_or_interface_type` uses the canonical bound kind and keeps
the first bound association only as a diagnostic location. Existing
`outer_type_parameters_of_symbol` remains the sole owner of native
`getOuterTypeParametersOfClassOrInterface`'s ValueDeclaration selection, after
the target is published as required by `getDeclaredTypeOfClassOrInterface`.

The distinct `getBaseTypeNodeOfClass` question continues to use
`GetClassLikeDeclarationOfSymbol`'s first class-like declaration, rather than
substituting ValueDeclaration. Its `class_recipe` now consumes complete native
declaration facts and resolves the selected node by kind and full span through
`reference_declaration_identity`. This also supports anonymous classes without
introducing a name-token fallback. The unused declaration field is removed from
that recipe. These changes are staged and source-reviewed; the root owns the
post-fix validation receipt.

Stage 41 proved private access GREEN
(`vrun-33acdaf9-4e70-415a-8fba-112a77856ba5`) and template types GREEN
(`vrun-f66bf7e7-34b0-45ea-a631-636ae8fdeb31`). Output and equality advanced to
`parameter_possibly_referenced`, whose parameter-origin recipe still projected
`tp.symbol.Declarations` into a DeclId-only list and reattached the sole entry
by anchor. Native `isTypeParameterPossiblyReferenced` at checker.go22309 asks
for the complete declaration list, then the sole actual declaration's Parent;
for synthetic this parameters that declaration is the class itself. The next
staged fix uses existing `symbol_declaration_references` followed by
`resolve_declaration_reference`, retaining native cardinality and kind/full
span. Its ancestry walk, TypeQuery scope and three evaluator callers are
unchanged. This follow-up has not yet received a final-query GREEN receipt.

### Proposed source and enum literal completion

This approved source-derived migration is staged with final-query tests;
its combined Rust validation remains root-owned. The operating envelope remains retained TypeScript source and
request-local semantic queries; no external encoding, process-ID spelling or
additional public literal category is needed.

Two small designs were compared. Keeping `ExpressionForm::Literal.value` and
enum publication scalar while adding special lone-surrogate branches would
split one literal category and leave every later consumer responsible for its
conversion. Widening only those fields while retaining UTF-16 constants is
also insufficient: it pairs independently concatenated halves before the
native operation asks for normalization. The coherent alternative is to carry
the existing `LiteralValue` through source literal recipes and native constant
string results, converting at each cooked source-token boundary and preserving
exact bytes through concatenation. The chosen representation hides encoding
and concat mechanics in existing owners; native normalization remains an
explicit decision at its actual caller.

The bounded migration is:

1. `source.rs::ExpressionForm::Literal` carries `LiteralValue`. String and
   no-substitution template literals admit cooked lone halves; numeric,
   Boolean and bigint recipes retain their existing scanner/normalization
   behavior. `evaluation/expression.rs` routes strings through the existing
   exact regular-string interner followed by `fresh_literal`, as native
   `checkExpressionWorker` does. Numeric and bigint checks retain their scalar
   requirements. `getTypeFromLiteralTypeNode` already regularizes that shared
   expression result, so declaration, expression and literal-type queries do
   not need separate string branches.
2. `constants.rs::ConstantValue::String` stores `LiteralValue`, and
   `AnyToString` returns it. `source/constants.rs` converts each cooked token;
   `evaluation/constants.rs` uses exact concat for binary plus and template
   constant evaluation. Native `evaluator.NewEvaluator` and
   `evaluateTemplateExpression` concatenate Go-string bytes without calling
   `CombineSurrogatePairs`. Their result must retain adjacent sentinel halves.
   The constant early-return branch of `checkTemplateExpression` must publish
   that exact value without normalization. The separate
   `getTemplateLiteralType` factory and matcher `addMatch` keep their existing
   explicit `CombineSurrogatePairs` calls.
3. `Model::enum_literal_type` accepts the exact value directly, preserving its
   existing enum identity/value interner and NaN rule. `evaluation/enums.rs`
   passes constant strings without scalar projection. `relations/enums.rs`
   compares the exact constant payload and uses `printer::quoted_bytes` for
   native `ValueToString` diagnostics, mapping any impossible identity demand
   through the existing typed failure owner. Source constants have fixed bytes;
   no generated identity enters this evaluator. Number formatting is unchanged.

The native probe confirms a scanner boundary that must be migrated before
claiming source strings exact. Native `scanEscapeSequence`/`scanLowSurrogateEscape`
combine immediately adjacent high/low Unicode escapes, including braced forms;
`scanString` and `scanTemplateAndSetTokenValue` concatenate their returned bytes
without a final combine. The former `parser/literals.rs::decode` accumulated
UTF-16 and globally paired it, losing the continuation boundary before semantic
admission. `LiteralValue::from_utf16` cannot recover that history.

The staged decoder now owns native-byte cooking, scanner flags and error state.
A borrowed byte result preserves the plain-token fast path. Public AST text
projects those bytes to the established Text/Utf16 representation. A private
Node payload retains native bytes only when that projection loses their spelling;
ordinary scalar tokens reuse public Text storage. `Ast::cooked_literal` borrows
that retained payload or scalar Text without decoding, allocating, or guessing
scanner dialect. JSX's existing raw attribute producer therefore stays distinct.
The rejected rescan-on-demand alternative would repeat large-token work without
request accounting and make every consumer responsible for scanner mode.

Token delimiter trimming belongs to the parser decoder; semantic recipes never
interpret TemplateHead/Middle/Tail. Ordinary source rejects unterminated tokens before literal publication, but
JSDoc recovery admits them. The delimiter owner consumes the scanner Token and
retains every byte after the opener when that token is unterminated, including
constructor-name lookahead; it never guesses termination from the final byte. Controlled decoder allocation, byte traversal, speculative
low-surrogate lookahead and public projection use the existing request ledger.
A failed speculative low read restores offset/flags but retains work charges.
The measured final-query resource test covers work/memory immediately below,
at and above the healthy request, repeated healthy requests after failure, and
long flat tokens with the same peak grammar depth. No public AST category or
second escape parser is introduced.

Exact identity adapters in `binding/text.rs` must use the exact cooked bytes
and reject non-scalar values, including continuation-separated high/low halves.
The public UTF-16 projection can pair those halves and therefore cannot answer
the native name-identity question. Public parser regressions retain that existing
projection, while isolated Project::load tests prove rejection at each actual
export/module literal. Property names retain their query-demand failure
boundary, with exact member-key retention preventing an earlier false merge.
Scalar and immediately paired names remain
admitted. This is one decoder decision with separate public-text and exact-name
projections, not permission to change the public AST text contract.

The six-profile `native-string-values` bootstrap
`vrun-21462312-6405-466b-9969-667b2d77776f` completed all 84 native queries with
hook/API parity and no demand failures, then failed only for missing baselines.
Its rows prove all four adjacent fixed/braced escape combinations become a
supplementary scalar; every removed LF/CR/CRLF/U+2028/U+2029 continuation retains
two sentinel encodings and compares unequal to that scalar. Constant-template
concatenation likewise retains sentinel halves, while template-type construction
combines them. All four casing mappings, expansion and Final_Sigma controls
are included. Maintained replay is GREEN
(`vrun-017e3497-1a1f-485d-9077-e0feadffafe1`): six profiles executed and passed,
with zero skips.
The actual call prelude still emits enum relation diagnostic 2345 under noCheck;
the test expectation follows that reached native schedule. These invalid calls
must retain OverloadRecovery, rather than fail on a scalar text projection.

The separate `native-string-names` fixture has 27 property/export/module queries
per strict/loose profile. Native conditional results expose the non-scalar name
distinction and exact ambient imports establish lookup identity. Maintained
replay is GREEN (`vrun-ae08d8bf-b1f3-47a0-bb17-868d19357380`): both profiles
executed and passed, with zero skips. Rust tests are staged in
`type_native_string_values.rs`, `native_string_names.rs`, and `literals.rs`; the
root owns their RED/GREEN checkpoints before production release.

The shared cooked-token consumer audit also found `parser/json.rs::string`:
configuration and JSON-module keys currently reconstruct strings from the public
AST projection. It now borrows the parser-owned exact cooked value and retains
non-scalar native bytes in `JsonString::NonScalar`, leaving existing config/name
admission decisions in their owners. Keeping UTF-16 for this private carrier
would still merge different scalar/sentinel partitions inside non-scalar keys. The strict package manifest
reader is a different native question (`encoding/json`) and stays separate.
The 27-project `native-string-json` fixture maintains that distinction with
GREEN `vrun-5148e003-c2e3-4e5a-96e2-611fa1ae57aa`. Config lone/continued names
produce TS6053 and select neither an existing scalar nor replacement-character
file; paired/scalar controls load the actual file. A JSON scalar-key import
retains only the first declaration when the second key contains sentinel halves,
and both declarations for paired/scalar duplicates. Malformed strict manifests
take index.d.ts fallback; paired/scalar manifests take the actual types file,
and none selects the replacement-character file. Rust final-load/query tests for all 27 projects are staged. A separate 18-row
fresh type-query native capture adds the actual imported value summaries with
globals explicitly supplied; native evidence alone does not prove product closure.

Native fixture variants must distinguish lone high/low, reversed halves,
adjacent fixed/braced escape pairs, supplementary scalar controls, and halves
separated by each removed line-continuation form. They must also distinguish
binary constant concat, constant template expressions and template types,
including equality and inferred captures that expose bytes instead of relying
only on coincident display summaries. Cover all four mappings, declaration and
expression values, literal-type annotations, enum member values, equal and
unequal same-named enum relations, and the reporting branch that quotes
differing enum values. Public output should assert existing `String | {parts}`
JSON: valid Unicode remains text, lone/adjacent sentinels remain exact byte
parts, and no symbolic IDs or origins appear for source-only constants. Update
the obsolete lone-surrogate Unsupported controls only after native capture and
Rust RED, with strict/loose and cold/warm final queries.

Enum element-name conversion in `evaluation/constants.rs` and direct/computed
property-name scalar gates answer name lookup questions. Enum constant lookup
now delegates its reached argument-name demand to the existing source name
owner, as detailed below. This preserves the non-scalar identity exclusion;
literal-value admission does not imply non-scalar property-name admission or
complete composed property ordering.

### Exact literal ordering and the property-key boundary

Stage 43's compact output and equality queries establish exact literal values,
shared/distinct identities and declaration origins. Expanded tuple members
then ask for the tuple's Array base, which builds a union of its element types.
Pinned `checker.go::addTypeToUnion` at 25710 binary-inserts with `CompareTypes`
before union reductions. `utilities.go::CompareTypes` compares type flags and
names first, plain string values at 490 and template spans at 543, and only then
falls back to type IDs. Extracted identity literals have no name symbol that
can bypass their byte comparison. Distinct opaque decimal values therefore
have an irreducible lexical-order demand even when semantic inequality is
already known.

Two designs were compared. Extending property-key B ordering with native byte
comparisons for composed values is inconsistent: unique keys with underlying
names `a` and `a0` order by those names, but a composed `FE@a9` followed by an
atom creates the cycle `a < a0 < composed < a`. A separate literal operation
answers the actual native string question and keeps the existing key policy
intact. The root approved this correction to the earlier reuse proposal.
Previously admitted literal unions that relied on identity ranks can now fail
with `UnrepresentableIdentity` when their actual decimal order is needed.

The staged comparator consumes fixed bytes and equal whole atoms from the
left. It stops at the first decisive comparison or spelling dependency; unlike
semantic equality it cannot cancel a suffix past unknown earlier digits.
`UnionFailure::Literal` preserves the typed cause until its actual consumer,
using the same located failure mapper as other literal observations. No new
property-key family rule or identity observation is introduced.

Final-query tests retain Found for compact and depth-zero distinct-ID tuples,
and require Partial with the exact `UnrepresentableIdentity` issue, tuple
annotation location and four blocked member collections when expansion
reaches their union. Their exact element values and origins remain asserted.
The hidden-summary case still requires expanded Found: its fixed `x` prefix
decides ordering against a positive-decimal atom. Comparator invariants cover
fixed FE versus U+FFFD, end/prefix, equal atoms with differing tails, all first
digit possibilities, unequal atoms, adjacent partitions, and the concrete
three-value cycle above. These are staged checks; root owns validation receipts.

The independent `native-generated-literal-ordering` fixture adds 28 final
queries per strict/loose profile. Each ordinary tuple retains its identity
beside an inline union, keeping the compiler's observed IDs and order in one
query without assuming a digit value. It covers both insertion directions for
empty, NUL, `0`, `:` and the three-value cycle; shared-atom suffix/prefix and
repeated-atom comparisons; fixed prefixes; raw FE versus U+FFFD; all nine
positive first-digit overlaps; unequal atoms and adjacent partitions. An
abstract-template union is consumed to the ordinary literal `ordered` so its
span comparator is exercised independently of unfinished symbolic template
presentation. The harness compares hook/API outcomes and retains actual native
summaries, selected locations and declaration metadata. Its bootstrap capture
and maintained replay precede the new Rust final-query test; existing Rust
files stay frozen during the root's checkpoint.

The root's Stage 45 output (`vrun-ee9e6a88-afa5-42e3-b0e1-93b88afeb8d2`)
and equality (`vrun-b80f064a-e9c3-4236-a571-5beed7f6fd96`) checks are GREEN
with exact ordering. The new ordering fixture's maintained native replay is
GREEN (`vrun-a08da1bf-5781-464b-b4a0-eb7a8fa5c537`): both profiles, all 56
query rows, no skipped tests. `tests/type_generated_literal_ordering.rs` now
asserts the final ordered union alternatives and exact public literal values,
with explicit reached digit failures; its Rust receipt is still pending.


The typeof witness cache now stores exact LiteralValue source bytes. Native
`getSwitchClauseTypeOfWitnesses` distinguishes continuation-separated halves
from a scalar while treating an immediate escape pair as the scalar. The two
profiles have five final query rows each: the distinct second cases narrow to
object, the duplicate paired case to never, and the escaped standard name to
string. The same harness proves public JS/JSX string-token text differs as the
scanner modes require. Those controls passed in `vrun-5ed2efc8-8131-4f05-b2b3-bcfa62942e7d`;
the combined run was RED only because the new JSON type fixture lacked globals.
No Rust decoder pass is claimed before the root's next checkpoint.

The maintained combined native replay is GREEN
`vrun-b7e47f10-dff1-420c-bccc-e1de8e6e9890`: all seven registered harness tests
executed and passed, zero skips. It covers the 27 config/JSON/manifest projects,
18 fresh imported-value type queries, 10 typeof-switch rows, the public JS/JSX
AST control, and 24 independent module-export selection rows. JSON paired/scalar
duplicate keys emit 1117 during the fresh query but return ordinary boolean,
without errorType or consumed recovery. Root confirmed the existing diagnostic-
only grammar policy: these product queries remain Found. Distinct sentinel-key
variants return number. Manifest malformed/non-scalar variants return 3 from
index.d.ts; paired/scalar paths return 1 from the exact supplementary-name file.
Rust decoder production and tests are CLOSED for the combined checkpoint; no
Rust validation receipt is claimed by this lane.

Stage 50's bounded peer review found JSDoc recovery admits malformed scanner
tokens, invalidating the first delimiter helper's terminated-token assumption.
The corrected token-body operation receives the whole scanner token in both
literal publication and constructor lookahead. It removes a closing delimiter
only from a terminated token. The final AST regression covers 38 native cases:
both quote styles, empty/ASCII/multibyte bodies, escaped closing quotes, LF/CR/
CRLF/comment-end termination, and initial/tail template tokens. The Ast cooked
value operation now independently borrows its supplied Node rather than
unnecessarily requiring the Ast and Node source lifetimes to coincide. These
narrow fixes await the root's next Rust compile.

Stage 50b compiled the decoder and passed ordinary source/enum values and all
public literal AST controls, including malformed JSDoc. The first typeof and
JSON assertions had assumed native success implied product admission. Their
failures instead match existing explicit limits: a native changed read is
FlowRequired, and JSON module construction demands every actual property key
before named selection, so a non-scalar key is UnrepresentableIdentity at that
key even when the selected scalar key is distinct. Exact JSON bytes prevent a
false merge but do not admit non-scalar property identities. The product tests
now preserve these located failures. Inert declared-object/string switch twins
make exact witness identity observable as Found versus duplicate ordinaryNever
requiring flow; no production flow or JSON policy was widened.

The expanded typeof witness native baseline has ten rows per strict/loose
profile, with hook/API parity, no errorType and no acquired diagnostic or demand
failure. The five inert twins preserve declared object/string except the
immediate-pair duplicate, whose ordinaryNever still needs excluded flow. This
matrix directly distinguishes byte-exact witness identity from UTF-16 pairing
without requiring product flow support.

Stage 52 closes the remaining enum constant argument-name classification gap.
Native `evaluateEntity` resolves the entity receiver and tests its Enum flag
before reading `ArgumentExpression.Text` for the raw export-table lookup. The
source recipe now retains that actual argument expression. At the same reached
point, constant evaluation calls `Source::try_property_name_text`, whose shared
scalar-name rule reports `UnrepresentableIdentity` at the argument. It no longer
projects a literal value through the generic literal-error mapper at the outer
consumer. No new decoder, name policy, lookup fallback, or eager enum-name
validation is introduced.

The independent enum argument fixture covers lone high/low surrogates, halves
separated by LF/CR/CRLF/LS/PS continuations, and adjacent-pair/scalar controls in
both strictness profiles. All 18 fresh native queries produce `"one"` without
diagnostics or recovery; the bootstrap run failed only for missing baselines.
The Rust final-query test requires Found for scalar controls and the exact
argument-located identity failure for excluded names, in both query orders and
with repeated compact/expanded demands. Maintained native replay is GREEN
(`vrun-fb0dade0-55dc-44d2-9e91-b3407b85d326`): both profiles executed and passed,
zero skips. The root's Rust checkpoint is pending. Bounded peer review found no
schedule or owner defect; no-substitution-template argument controls will extend
this fixture in the next open test window.

The older `type_switch_flow` unpaired-case negative now follows its retained
pinned observations: the case read changes declared `string` to the unpaired
literal and therefore requires excluded flow, while the default remains
`string` and is Found. It no longer expects a literal-representation failure
after the native-byte migration.
