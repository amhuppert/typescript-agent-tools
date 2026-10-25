# Construction sites

`--constructs` lists the object literals that construct a selected interface,
class or type alias, and which of its properties each literal does not
certainly provide, optional ones included:

```sh
tsparse --constructs --project tsconfig.json --name RunView
tsparse --constructs --project tsconfig.json --name RunView --missing lane --no-tests
tsparse --constructs --project tsconfig.json --at src/views.ts:12:18 --json
tsparse --constructs --project tsconfig.json --name Options --all-literals --json -o sites.json
```

Use it after adding or changing a property: a required property shows up as
one compiler error per construction, an optional one nowhere. The
[technical design](constructs-design.md) records the rules and their
TypeScript sources.

## Selection

`--name` takes `--show`'s declaration selectors, with an optional FILE that
narrows the seeds; `--at` selects a declaration's name. The selection must be
a class (its instance side), an interface or a type alias. Anything else,
including a type reference in an annotation, is `UnsupportedTarget`, as is a
declaration whose construction sites cannot be identified: one whose declared
type has no object type, whose properties depend on its type arguments (a
generic mapped type, or an intersection with a type parameter), or a generic
alias with an alternative whose instantiations keep no identity (such as an
intersection inside a generic union). Missing and ambiguous selections are
`not_found` and `ambiguous` documents, as for `--type`.

## Which literals are examined

By default a request examines the literals, in the searched files that the
filters admit, that the target's keys select (the `target_keys` universe):

* a literal with a member naming a property of some target alternative:
  `{id: 'a'}`, `{id}`, `{run() {}}`, `{get size() {…}}`, `{['id']: 'a'}`, or
  `{0x1: 'one'}` and `{[+1]: 'one'}` for a property `1`, as TypeScript names
  the literal's property (an accessor keeps its spelling: `{get [+1]() {…}}`
  names `+1`);
* a literal with no named member: `{}`, which constructs a target whose
  properties are all optional, and literals made only of spreads and computed
  names that are not literals (`{...base}`, `{[key]: value}`).

Only a computed literal name is typed to decide, never an initializer. A
literal whose named members all name other keys is not examined; the header
and `coverage` count it. Such a literal can still construct a target
alternative, for example:

* under a type assertion, which relates comparability rather than checking
  excess properties (`{other: 1} as Options` is valid when every property of
  `Options` is optional);
* in a union context beside the member those keys choose (`const p: Pair |
  View = {id, status}` constructs `View`, and `Pair` lacking all of its
  properties);
* in a union context whose other member accepts any key, so TypeScript
  checks no excess property (`View | {}`, `View | object`, `View |
  Record<string, unknown>`);
* with an excess-property error (TS2353), also beside a spread or a
  late-bound name (`const v: View = {...base, debug: true}`).

`--all-literals` examines every literal, these included. Without the flag,
every literal is also examined for a target with an alternative against
which any key is valid: one with an index signature, an empty object type
(`interface Empty {}` accepts `{meaningful: 1}`), or the global `Object`
type. `coverage.universe` is then `all`.

## Which literals are sites

A literal is a site when its contextual type, after TypeScript's own
discrimination of object-literal unions, is the target or one of its union
alternatives. The comparison reads the contextual type as declared and
instantiated, before TypeScript's apparent form rebuilds an intersection
without its alias, so `type PointExtra = Point & {extra: string}` and
`type Props = Box<string> & Plain` match the literals annotated with them:

* Non-generic targets are identities: `type Fixed = Box<number>` constructs
  only literals TypeScript types as `Fixed`.
* A generic interface, class or alias matches its instantiations:
  `Box<string>` and `Box<number>` are both constructions of `Box`.
* An undiscriminated union keeps every matching alternative; the site's
  `contextualType` shows the others. A literal typed by one alternative of a
  union target constructs that alternative.
* A type parameter's base constraint stands for it, and a class's `this`
  type for the class: `{id} as T`, with `T extends Plain`, constructs
  `Plain`. A literal whose context is a type parameter TypeScript infers
  (`const p: Plain = id({id})`) takes the inferred type, not the target.
* An alternative with call or construct signatures is never constructed, as
  no object literal has a signature; a target without another alternative is
  `UnsupportedTarget`.
* Base types, intersection parts and mapped forms such as `Partial<T>` are
  other types: query them by name.
* Annotations, assertions, `satisfies`, returns (including `async` and arrow
  bodies), arguments, array elements, nested property values, defaults and
  assignments all supply contextual types as TypeScript does.

Two kinds of literal are never sites: destructuring patterns such as
`({lane} = value)`, and a literal whose context comes from an enclosing
literal's spread (`{id, ...(lane ? {lane} : {})}` is one site, whose `lane`
is `unknown`).

## Presence

For each constructed alternative, `lacks` lists the properties the literal
does not certainly provide, in the alternative's order, with `optional` from
the alternative. Presence is read from the compiler's type of the literal:

| Presence | Meaning |
| --- | --- |
| `absent` | The literal's type has no such named property, no index signature applies to the name, and the global `Object` type (with its augmentations) has no such member. |
| `unknown` | It is optional there (an optional spread, or an optional member of an augmented `Object`), an index signature applies (a computed key or a spread of a record), it is present in only some alternatives of a union (a conditional spread), or the literal's type is `any` (a spread of `any`). |

A property the global `Object` type declares as required, such as
`toString`, is present in every literal, as TypeScript's property lookup
finds it; only `Object`'s own named members count, not its signatures or
index signatures.

This is the compiler's model, not the runtime object: spreading a value of
type `{}` provides no named property even when the value carries one. A later
spread can also hide an earlier written key, as TypeScript's spread rules do
for private members.

`--missing KEY` keeps the sites with a constructed alternative that declares
the string key `KEY`, compared exactly (the empty string included), and does
not certainly provide it. A key that no alternative declares as a string key
fails with `UnknownProperty` (exit 1), suggesting the nearest key.

## Output

Text names files absolutely: a header with the selection, the site and file
counts, the filters, the literals examined (`68 of 98` when the universe
left some) and the coverage; the target and its declaration; then one line
per site:

```text
/work/src/runs/project.ts:40:10 lacks "lane"?, "status" (unknown)
/work/src/runs/host.ts:88:16 in Other | RunView lacks "status", "lane"?
/work/src/fixtures.ts:12:24 as Box<string> lacks "label"?
/work/src/secret.ts:3:20 lacks private "#hidden", symbol "id"
```

Keys are quoted as `--type` quotes them, so a key's own `?`, parentheses or
newline reads as part of its name, with `private` or `symbol` before a key
that is not a string; `?` after a key marks an optional property;
`(unknown)` an uncertain presence; `in` names the contextual type when it
differs from the constructed alternative, as a union's other alternatives
do; `as` names the constructed alternative when it differs from the target.
The JSON `contextualType` is the instantiated contextual type, or the
discriminated apparent type when discrimination removed a union member,
which it always does for `undefined` and `null` (`RunView | undefined` shows
`RunView`). After the sites and gaps, one line counts the literals that were
not examined and names `--all-literals`. `--json` writes the result
document, with locations relative to the configuration's directory and each
literal's whole span.

At most 200 sites, 20 lacking properties per construction, and 50 gaps are
shown; `--limit N|all` sets all three, and `-o FILE` writes everything. A cut
is the last text lines and `display` in JSON (`omittedLacks` on a cut
construction); it never changes `outcome` or `coverage`.

## Coverage and failures

`coverage.universe` is `target_keys` or `all`, `coverage.literals` counts the
literals in the files searched, `coverage.examined` those whose construction
was asked, and `coverage.sites` the sites. `filters.allLiterals` echoes the
flag. A literal whose context or presence cannot be established is a gap
located at the literal, with Q3's reasons, and the outcome is `partial` (exit
1). Common gaps are reads that need flow narrowing (`flow_required`) in a
call's other arguments or in a spread literal's members, JSX attribute
contexts, assignments to properties, and invalid spreads. Memory, time and
internal failures end the query, as for `--type`; the same
[process limits](types.md#process-protection) apply.

## Cost

A request parses every searched file and types the context of each examined
literal, so it costs more than a type query; most of the time goes to
argument contexts, which resolve the enclosing call. On a project of 4,275
files and 110,159 literals, the default universe examined 6–14% of them; a
warm service answered in 14–23 s, and a direct run took 21–31 s including
about 7 s of loading. `--all-literals` took about 74 s and 6.1 GB there,
past the default 60 s deadline and near the memory threshold
([process limits](types.md#process-protection)): narrow it with `--file` or
`--no-tests`, or raise `TSPARSE_QUERY_TIMEOUT_MS`. The
[measurements](constructs-design.md#measurement-gate) are recorded in the
design.
