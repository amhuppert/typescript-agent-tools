# Statement sites and the test view: technical design

Status: proposed, 2026-10-25, from tsparse friction a Codex agent reported in
remote-ai-manager ("Improve Docs Tab Features"). Reviewed the same day
against commit `c4a9350`; every statement below about current behavior was
checked against that code and its build. The user selected two
capabilities: **statement sites** (an `--at` position on a statement's first
token selects that statement for `--show` and statement insertion) and the
**test view** (a test file's `describe`/`it` hierarchy with each test's name).
This document records the design and its rationale. The behavior contract,
once implemented, lives in [navigation](navigation.md) and
[editing](editing.md).

## Operating envelope

The same as navigation and editing: one local user, a one-process-per-request
CLI with the optional per-user project service, TypeScript 7.0.2 syntax, and
UTF-8 TypeScript and TSX files. The test view's callers are agents reading
Vitest-style suites. In remote-ai-manager, the motivating repository, that
means 1,677 test files, all importing from `vitest`. They use `it.each` 611
times, `describe.each` 21 times, and hooks (`beforeAll(` and the others)
1,534 times. They await no test call: each of their 229 `await test.`
occurrences calls a fixture object named `test`. They use no Playwright, no
`suite`, no tagged-template `.each`, and no `.shuffle`. (Counted in its main
checkout at `c9523669c`: the files `git ls-files` lists for `*.test.ts`,
`*.test.tsx`, `*.spec.ts`, and `*.spec.tsx`, which excludes the copies in
its worktrees, with word-bounded `grep -o` counts.) A large file has a few
thousand lines and under a few hundred tests. Nothing in this design runs
concurrently, persists state, or crosses a trust boundary. Both capabilities
are syntactic, so the test view needs no project and no service.

## Problem

Two reported failures, reproduced on remote-ai-manager's files:

- **No way to insert after a test.** The outline lists a test only as its
  callback, `134:75 function it("keeps the selected file…") callback`.
  Insertion refuses callbacks (`UnsupportedSite`), and `--at 134:3` on the
  `it` itself selects nothing ("no declaration is named or anchored at this
  position"). Inserting after any non-declaration statement, such as an
  `expect(...)` line, is impossible for the same reason. The only working
  path is to replace the whole enclosing `describe` callback's body.
- **The outline hides test structure.** On `machine.test.ts` (2,131 lines,
  66 tests), the default outline (114 lines) lists every `describe` but no
  `it`. Every level (`--depth all --limit all`) is 1,072 lines, of which the
  tests are about 6%; with only `--depth all`, the 200-entry display cap cuts
  the list after 169 lines, which show 2 of the 66 tests.
  `it.each([…])("adds %i", …)` and `describe.each(…)(…)` become
  `<function>`, `describe(Widget.name, …)` becomes `describe() callback`,
  and `it.todo("…")` has no entry. These
  labels follow TypeScript's document symbols (`getUnnamedNodeLabel` and
  `getCallExpressionName`), which the outline ports on purpose.

## Statement sites

### Contract

#### Selection

`--at LINE:COLUMN` turns the position into an offset as today; a position
outside the file, past the end of its line, or inside a surrogate pair stays
`InvalidPosition` (exit 2). Then:

1. The outline entries selected today are selected: each entry anchored at
   the offset, or any of whose `--at` targets contains it. An entry's targets
   are its name tokens (each segment of a dotted namespace, and a computed
   name's whole `[…]`), its anchor token, and a constructor's `constructor`
   keyword. One entry is found; several are candidates. This step is
   unchanged, so **every position that selects an outline entry today keeps
   exactly today's selection**: the same entry or candidates, so `--show`
   gives the same text, document, and exit code, and an edit selects the same
   site. The only output that changes at such a position is the callback
   insertion refusal, which gains its remedy (below).
2. Only when no entry matches: if the offset lies in the first token of a
   statement, from the token's first character up to but not including its
   end, that statement is selected as a **statement site**. A statement is
   any node of the final syntax tree that `syntax::is_statement` accepts, and
   its first token is `Ast::first_token_span` of that node.
3. Otherwise nothing is selected: a missing selection, with today's text
   (`no declaration is named or anchored at this position`) and exit 1.

`--name` is unchanged: a statement site has no key. A statement site is
selected only alone, so it is never among an ambiguous selection's
candidates.

A position anywhere in the first token selects: on the Examples' line 10,
`  it.skip("skipped one", () => {});`, `--at 10:3` and `--at 10:4` (both in
`it`) select the statement, and `--at 10:5` (the `.`) selects nothing. A
statement's first token is the first token of its range as `--show` already
defines ranges: decorators and modifiers included, leading comments and
JSDoc excluded. `first_token_span` lexes from the statement's start without
the parser's rescans. So a statement that begins with a regular expression
or JSX has as its first token the punctuator the lexer scans there, not the
literal: `/` or `<`, or `/=` for a regular expression that starts with `=`.

**A token is the first token of at most one statement.** Two statements with
one first token would have to nest, the inner one beginning at the outer
one's first token. No kind `is_statement` accepts begins with a nested
statement:

- Most kinds begin with a token they own: a keyword (`if`, `for`, `do`,
  `return`, `var`, `let`, `const`, `using`, `await using`, `function`,
  `class`, `interface`, `type`, `enum`, `namespace`, `module`, `import`,
  `export`, …), `{` for a block, `;` for an empty statement, or a modifier or
  decorator before the keyword. Modifiers and decorators are the statement's
  own, and a decorator begins with `@`, so `@dec export class C {}`,
  `export default function () {}`, `export default x;`, `export = x;`,
  `export import a = b.c;`, and `export as namespace N;` are each one
  statement.
- A labeled statement begins with its label, an identifier, not with its
  body: `a: b: f();` holds three statements whose first tokens are `a`, `b`,
  and `f`.
- An expression statement begins with its expression's first token, and no
  expression begins with a statement: statements inside an expression lie in
  a function body or a class static block, after the function's or class's
  header.
- `namespace A.B {}` parses `B {}` as a nested module declaration whose first
  token is `B`, after `namespace A.`.
- A block is a statement unless it is a function body or a `try`, `catch`, or
  `finally` block; those blocks' `{` is the first token of no statement. A
  class static block's body is a block whose parent is not function-like
  (`is_function_like_kind` omits static blocks), so it is a statement, and
  its `{` follows `static`.
- An unbraced body is a separate statement after its owner's header:
  `if (a) if (b) f();` holds three statements whose first tokens are the two
  `if`s and `f`.
- `MissingDeclaration` occurs only in a file with syntax errors, which
  navigation refuses, and `NotEmittedStatement` only in emitter output.

So the statement step needs no tie rule. Because step 1 comes first, a
statement whose first token is an entry's anchor or lies in an entry's name
token keeps the entry's answer:

- `export default function () {}`, `export default class {}`, and
  `export = f(…)` have entries anchored at `export` that designate the
  statement itself, so insertion after them already works.
- An expression statement that begins with an arrow function, `x => {};` or
  `async () => {};`, has a `<function>` entry anchored at its first token; no
  position selects that statement.
- A statement inside a computed property name, as in
  `[(() => { f(); })()]() {}`, lies in the member's name token, so `--at` on
  `f` selects the member.

A consequence for imports and exports: `--at` on `import` in
`import { a } from "m"`, or on `export` in `export { a }`, matches no entry
(specifier entries are anchored at their names) and selects the declaration
as a statement site, the excerpt `--at` on `a` already prints. This answers
the reported `--show --at 1:1 MarkdownDocumentList.stories.tsx`, and the
imports design (the session's item 2) takes it as given.

#### The statement site

A statement site is reported as a `Declaration`, as every selected site is:

- `kind`: `statement`, a new `Kind::Statement`, rendered `statement` in text
  and JSON.
- `label`: exactly what the signature renderer (`signature::render`) gives
  for the statement's range without a final `;`, with no name. Each run of
  whitespace and comments between tokens becomes one space. Function block
  bodies and class member lists become `{ … }`. A string, template, or JSX
  text literal that spans lines becomes `…`. Past 160 characters the text is
  cut at the renderer's last boundary and ends with `…`. Its boundaries lie
  only before a space it writes and on either side of a literal or elided
  body, and with no name the cut may fall at the start, so a statement whose
  first 160 characters hold none of these (a long `a.b().c()…` chain)
  renders as `…` alone, as such a signature already does today.
  This is the one label that abbreviates: the rule that labels are never
  truncated protects names, which are identities, and a statement site has
  none. Example: `it.skip("skipped one", () => { … })` for the Examples'
  line 10.
- `anchor`: the start of the statement's first token.
- `range`: the statement's range, from its first token through its last,
  including a final `;`; the excerpt is that range's source.
- `key`, `selector`, and `signature`: absent.
- `depth`: one more than the greatest depth among the outline entries whose
  range contains the statement's range and differs from it, or 1 when there
  is none. `--show` does not print depth; edit documents and plans, which
  serialize the whole site, do.

#### Outcomes by operation

| Request at a statement site | Outcome |
| --- | --- |
| `--show --at` | `found`, exit 0. Text: `FILE:LINE:COLUMN statement LABEL`, then the excerpt's lines numbered as today. JSON: `declaration` with `anchor`, `kind` (`"statement"`), `label`, and `range`; `source` is the excerpt. The `lines` display cap applies as to any excerpt. |
| `--insert-after --at`, and a batch `insert_after` whose `at` selects the site | The site designates the statement. A statement in a file, block, module block, or `case` or `default` clause takes the insertion after it, and the preview line reads `FILE:LINE:COLUMN: insert_after statement LABEL: after the statement at L1:C1-L2:C2`. A statement elsewhere, the unbraced body of an `if`, `else`, loop, `with`, or label, or a class static block's body, is `UnsupportedSite`: `LABEL cannot have statements inserted after it: this statement is not a statement in a statement list (statements can be inserted only after a statement of a file, block, module block, or case clause)`. That refusal names no remedy. |
| `--replace-body --at`, and a batch `replace_body` whose `at` selects the site | `UnsupportedSite`, also when the statement is a function declaration: `LABEL cannot have its body replaced: a statement site has no body to replace; select the function, method, or arrow function by its name or anchor`. |
| A saved plan's replay (`--apply`) | Each edit is prepared again at its site's anchor, which selects the same statement site (see Ownership, "Replay"), so a plan of statement-site edits loads and applies as any plan does. |
| `--project`, with or without `--direct` | The same selections and outcomes as without `--project`, with files named as project results name them; the service's and `--direct`'s answers are byte-identical. `--show --at` re-projects the captured file into a complete catalog, and project edits select in this process during `editing::bind`. |
| `--name` | Unchanged. |
| `--type`, `--references` | Unchanged: their positions select tokens through semantic selection, not catalog sites. |

In a batch, every rejection is prefixed `edit N:` in text, as today, and an
edit's `at` takes the same `LINE:COLUMN` (with `file`) or `FILE:LINE:COLUMN`.
Two insertions after one statement conflict, as two insertions at one place
already do.

#### The callback refusal's remedy

"Never climbs" stays: a selected callback still designates itself, and
insertion still refuses it. The refusal now names the position that selects
the enclosing statement, by this rule:

- **Which refusals.** An insertion (`--insert-after`, or a batch
  `insert_after`) refused at an outline entry whose own syntax is a function
  expression or an arrow function: a callback, an assigned or returned
  function, or a default-exported arrow. Such a site designates itself, which
  is never an item of a statement list. Every other refusal is unchanged,
  including class expressions, members, loop and `catch` bindings, parameter
  properties, and declarations and statement sites in unbraced bodies.
- **The statement.** S is the innermost statement containing the function:
  its nearest ancestor in the syntax tree that `syntax::is_statement` accepts.
  Function bodies and `try`, `catch`, and `finally` blocks are not
  statements, so S is never one of them.
- **When it is named.** Both must hold: S is an item of a statement list (the
  same `statement_list` check insertion uses), and `--at P`, where P is the
  start of S's first token, selects exactly one site, which designates S. That
  site is S's statement site, or an entry that designates S, such as
  `export = f(…)`'s `export=` entry.
- **Text.** The refusal message then ends with
  `; to insert after the enclosing statement, select it with --at LINE:COLUMN`,
  where `LINE:COLUMN` is P, with a one-based line and UTF-16 column like every
  printed location. Otherwise the message is unchanged. Only S is considered;
  no statement outside it is ever named.

For the `it.each` callback in the Examples:

```text
<function> cannot have statements inserted after it: this function is not a statement in a statement list (statements can be inserted only after a statement of a file, block, module block, or case clause); to insert after the enclosing statement, select it with --at 5:3
```

The text output prefixes it with `FILE:LINE:COLUMN: edit N: ` as it does
every rejection, and the JSON `error.message` is the same string. No remedy is
printed for `if (c) f(() => {});` (S is the unbraced `if` body), for `x => {};`
(`--at` on S's first token selects the arrow again), or inside a computed
property name. As with every advertised remedy, a test follows the printed
position.

### Ownership

- **Selection:** `Catalog::select` (`src/navigation/catalog.rs`) is the one
  selection owner. It serves file `--show` (`navigation::file`), project
  `--show --at` (`Project::show`, through `Index::catalog`), and every edit,
  whether single, batch, project-bound, or replayed, through
  `editing::prepare`. The fallback belongs inside it, after the entry match,
  so no caller can forget it. A `Catalog` holds no syntax tree. File
  navigation drops the tree before answering, `Index::catalog` returns only
  the catalog, and preparation keeps the tree beside the catalog in `Syntax`
  only for policy. So `Catalog::new` also builds the file's statement sites
  while the tree is alive: each statement's node, first-token span, range,
  depth, and label. Projection, the module that alone knows syntax, gives
  them in a second walk of the final tree.
- **Statement sites are not records.** They are a separate collection on the
  catalog, beside `records`, and nothing about records changes:
  - `projection::project` returns exactly the records it returns today, for
    `Detail::Complete` and `Detail::Selection` alike;
  - every record keeps its ordinal;
  - `Catalog::declarations`, which the default `--outline` prints, lists
    records only;
  - the project snapshot's `Index` is built as today and holds no statement
    sites.

  Record ordinals must not move. Project-mode `--show --name` finds its one
  match in the snapshot's index and reads the record at the same ordinal in
  the catalog projected again (`Project::show_name`). The only guard there is
  a debug assertion, and `Index::catalog` requires the two projections to
  agree ordinal for ordinal. A statement site added as a record would shift
  every later ordinal, so a release build would show the wrong declaration.
  It would also enter the default `--outline`. A catalog's own index, which
  name selection inside the catalog reads (including every name-selected
  edit), is likewise built from the records alone.
  `Catalog::select`'s result distinguishes a record, by its ordinal, from a
  statement site, by its place in the separate collection. `Catalog::found`
  and `Catalog::candidate` keep taking record ordinals, and a statement site
  gets its own counterpart of `found` (it never becomes a candidate).
  `navigation::outcome` turns a single selected statement site into `Found`,
  and `Syntax::site` gives it a `SiteSyntax`
  whose `node` and `designation` are the statement and whose `kind` is
  `Statement`. These changes stay inside the navigation module's selection
  types and the callers that read them. Per catalog, the statement sites cost
  one more walk of the final tree with an explicit stack, plus one label per
  statement, rendered only as far as the 160-character cut. Name search and
  snapshot memory are unchanged.
- **What a statement is:** `syntax::is_statement` (TypeScript `IsStatement`,
  with its parent-sensitive Block exclusion). It takes a parents slice
  indexed by node. `Ast::walk` yields bare node ids, so the statement walk
  records each child's parent as it pushes it on its explicit stack.
- **First token:** `Ast::first_token_span` (TypeScript `getFirstToken`). Its
  start is the start of the statement's range (`Ast::token_span`), so
  decorators and modifiers are included and trivia is not.
- **Label:** the signature renderer (`signature::render`, through
  projection's `render`), over the statement's range without its final `;`,
  as projection's `without_separator` trims a statement's range.
- **Insertion eligibility:** the existing `statement_list` check in
  `src/editing/policy.rs`, reached through `SiteSyntax.designation`, which
  for a statement site is the statement itself. No second membership rule.
- **Body replacement:** `policy::callable` refuses a statement site before
  its callable check, which would otherwise accept a function declaration
  statement's node.
- **Kind:** `Kind::Statement` joins the navigation kind vocabulary
  (`navigation::Kind`, serialized in snake case). A statement site has no
  key, selector, or signature.
- **Replay:** a plan replays each edit from its site's anchor
  (`Target::replay` in `src/editing/mod.rs`). A statement site's anchor is the
  start s of its first token, and the selection that chose it matched no
  entry at a position p in `[s, e)`, the span of that token. An entry
  anchored at s has that token as its first target, which contains p. Every
  target is one whole token (a name token, a namespace segment, an anchor
  token, or a `constructor` keyword) or a computed name's whole `[…]`. So a
  target that contains s contains the whole first token, and p with it. So no
  entry matches s, and replay selects the same statement. The prepare-time
  check that a site's anchor selects the site alone (`src/editing/prepare.rs`)
  compares selections, entry or statement, where it compares record indexes
  today.
- **The refusal remedy:** `policy::plan`, which owns `statement_list`, finds
  S and its list membership when it refuses a function or arrow expression.
  `prepare`, which holds the catalog, checks that `--at P` selects one site
  designating S, and appends the remedy to the end of the refusal message it
  already builds (`LABEL cannot have statements inserted after it: REASON`).
  P is printed with the source's position rendering, like every other printed
  location.

### Alternatives considered

- **Let a callback designate its call statement** when the call is a whole
  expression statement. Zero extra steps for the agent, but it reverses "never
  climbs" for a common shape and reintroduces the ambiguity that rule exists
  for: an agent selecting a `useEffect(() => …)` callback to add a line to the
  effect would get the line after the effect. It also changes what `--show`
  prints for every such callback.
- **Outline entries for call statements.** Departs from TypeScript's document
  symbols and adds an entry for every `expect(...)` line, or needs a policy
  for which calls count.
- **Keep the refusal and point to body replacement.** Leaves an agent
  resending a whole `describe` body to add one test.
- **Statement sites only for items of a statement list.** Selection would
  have to ask E2's membership question too, through an operation it shares
  with policy, and `--show` could not show an unbraced `return`. Selecting
  every statement and letting insertion refuse the unbraced ones shows every
  statement and asks the question once, where policy already asks it.

The chosen design adds one fallback rule to an existing owner, keeps every
current answer, and covers any statement, not only tests.

## Test view

### Contract

```text
tsparse --test-outline FILE [--limit N|all] [--json [--pretty]] [--output OUTPUT]
```

It lists a file's test calls in source order, indented by containment, one
per line. FILE is navigation input, as for `--outline FILE`. The operation
accepts:

- its own flag;
- `--limit`, `--json`, `--pretty` (valid only with `--json`, as for
  `--outline`), and `--output` with its alias `-o`;
- the global `--help` (`-h`) and `--version` (`-V`), which every operation
  accepts (`GLOBAL_FLAGS`) and which print the operation's help or the
  version as they do for every operation.

Any other flag, including `--project`, `--direct`, `--depth`, `--at`, and
`--name`, is a usage error, and so is stdin (`-`). As for every flag an
operation does not take, the registry decides this, and the error lists the
operations that accept the flag.

**What counts as a test call** is decided by syntax alone, with no import
check, so the view works with Vitest globals and without a project. A test
call is an expression statement whose expression is a call expression C.
Nothing may wrap C: no parentheses, `await`, `void`, or other operator. C's
callee is either:

- a **hook**: an identifier named `beforeAll`, `beforeEach`, `afterAll`, or
  `afterEach`; or
- a **test chain**, defined by node kinds:
  - an identifier named `describe`, `it`, or `test` (the root);
  - a property access `X.m`, where `X` is a test chain and `m` is an
    identifier named `only`, `skip`, `todo`, `concurrent`, `sequential`,
    `shuffle`, or `fails`;
  - a call `X.m(…)`, where `X` is a test chain and `m` is an identifier named
    `each`, `for`, `skipIf`, or `runIf`. Its arguments, the table or
    condition, are any expressions and are not part of the chain.

A name is the identifier's decoded name (`Ast::identifier_name`), compared
exactly. Nothing else forms a chain: an element access, a non-null
assertion, parentheses, a receiver such as `this.it`, any other name, or a
modifier in the other form, which is a called modifier that is not called
before C (`it.each("n", fn)`) or a plain modifier that is
(`it.skip(c)("n", fn)`). The rule reads node kinds and names only, so `?.` and
type arguments change nothing. A tagged template is not a call, so Vitest's
template-table form ``it.each`a | b`("n", fn)`` is not a test call either;
the `.each` tables the view includes are the called form. `it.each(table);`
alone is therefore not a test call, nor are `test.extend(…)`,
`test.runtimeHost.advance(…)`, `test.describe(…)`,
`await test.step(…)`, `suite(…)`, and `expect(…)`. The fixed modifier list
guards against fixture objects named `test`: remote-ai-manager writes
`test.runtimeHost.advance(…)` and `test.requests…` statements, which a chain
rule without the list would report as tests. `await` is not recognized,
because no test call in the envelope is awaited, and Playwright's
`await test.step(…)` is out of scope. Statements count wherever they are,
inside helper functions, conditionals, loops, and unbraced bodies alike, with
one exception. A test call inside a computed property name (the `[…]` that
names a member) is not listed, and neither is anything inside it.

The exception keeps the view's promise that every listed position works
directly as a statement-site `--at` for its own statement, and it leaves
every outline selection as it is today.
Selection's step 1 gives every position inside an entry's computed
name to that entry: in `[(() => { it("x", f); })()]() {}`, `--at` on that
`it` selects the method. A computed name is the only `--at` target longer
than one token. No other target can hold a listed position, which is the
start of the root or hook identifier. That identifier is no declaration's
name token. It is no unnamed site's anchor either: the nodes that begin
there are the statement, C, the chain's nodes, and the identifier itself,
and none of them is an unnamed site. So at every listed position, step 1
matches nothing and step 2 selects the test's statement.

**Each entry** has three parts:

- **Position:** the start of its statement's first token, which is the root
  identifier or the hook's name, since nothing wraps the call. `--at` there
  always selects the test's statement as a statement site (see the exception
  above): `--show --at` prints the whole test, and `--insert-after --at`
  inserts after it when the statement is in a statement list.
- **Call:** the hook's name, or the chain's root followed by each modifier
  in source order, each preceded by `.`, with `()` after a called modifier:
  `it.skip`, `it.each()`, `describe.each()`, `describe.concurrent.each()`,
  `it.skipIf()`. It is spelled with the decoded names that matched, so
  `\u0069t.skip(…)` prints `it.skip`.
- **Name:** for a test chain, C's first argument as written. That is its
  source text from its first token through its last (`names::source_text`,
  the owner of node text), with every line break (LF, CR, U+2028, and
  U+2029, `lexer::line_break`) removed, as the outline removes them from
  callback labels. Nothing else changes, and nothing is truncated: `"adds %i"`,
  `'x'`, `` `handles ${"x"}` ``, `Widget.name`. A call without arguments has
  no name, and neither does a hook. For `.each`, `.for`, `.skipIf`, and
  `.runIf`, C is the outer call, so the name is the template after the table
  or condition (`it.each(table)("adds %i", fn)` is named `"adds %i"`).

**Containment and order.** An entry's parent is the nearest test call whose
statement contains its statement in the syntax tree. All other syntax is
transparent, including calls that are not test calls, such as
`test.describe(…)`. An entry without such an ancestor is at the top level,
even when it sits inside a helper function. Entries are ordered by position.
An entry's statement contains its descendants', so a parent precedes its
children and the order is a pre-order. `Ast::walk` visits fields in storage
order, and the parser stores modifiers last, so it reaches a test call in a
decorator's argument after the class's members. The view orders by
position, not by walk order. Every level is shown; there is no `--depth`.

**Text.** The file's absolute path comes first. Then each entry gets a line:
two spaces per containment level (none at the top level), `LINE:COLUMN`, a
space, and the call; when the entry has a name, another space and the name
follow. Lines are one-based and columns are one-based UTF-16, as every
printed position is. Nothing else is printed.

**JSON.** One document:
`{"operation": "test-outline", "outcome": "ok", "file": FILE, "entries": [ENTRY, …]}`.
FILE is the absolute path. Each ENTRY is
`{"anchor": {"line", "column"}, "call", "name", "children": [ENTRY, …]}`:
`name` is absent when the entry has none, and `children` is always present,
empty for a leaf. The document lists exactly the entries the text does, so a
test call inside a computed property name, and everything inside it, is
absent here too (see "What counts as a test call"). `display` is added only when a cap withheld entries. The
renderer builds the document as a `serde_json::Value`, as the outline's
renderer does. It nests the library's flat entries (see Ownership) and does
not serialize them directly. `serde_json` is built without `preserve_order`,
so every object's keys come out in alphabetical order. The shared
`write_document` writes the document, and `--pretty` indents it as it
indents the others.

**Bounds.** `--limit N|all` caps the entries in source order, with a default
of 200. A cut keeps a prefix, so every shown entry's parent is shown, and JSON
nests the shown entries. With `-o` and no `--limit`, nothing is cut. A cut
text ends with the shared cut line:
`… N more entries; see all with: COMMAND`, followed by
`, or write them to a file with -o FILE` when the request names no `-o`
(`… 1 more entry` for one). COMMAND is the request as given, with
`--limit all` in place of any `--limit` value. A cut document adds
`"display": {"entries": {"returned": R, "total": T}, "next": {"argv": […]}, "truncated": true}`.

**Empty result.** A file with no test call prints its path and nothing else
and exits 0, and its document has `"entries": []`.

**Failures** are the outline's, through the same input owner. A file that
cannot be read is `Io` (exit 2). A syntax error is `Syntax`, printed as
`path:line:column: message` (exit 1). A file past the parser's grammar budget
is `ResourceLimit` (exit 1). A JSON, JavaScript, or other file is
`Unsupported` (exit 1). With `--json`, each of these is an `error` document
naming `test-outline`. A usage error is `CODE: message` (exit 2), and so is a
result that cannot be written. The registry's exit table reads: 0, the view,
empty for a file without test calls; 1, a syntax error, a file past the
parser's grammar budget, or unsupported input; 2, a usage error, a file that
cannot be read, or a result that cannot be written.

### Ownership

- **Name:** `--test-outline`, not `--tests`. The references operation already
  has a `--no-tests` flag that excludes test paths, and `--tests` would read
  as its opposite.
- **Recognition:** the test view answers a question TypeScript does not ask,
  so it is a deliberate addition with its own owner, a navigation module
  beside the outline. `docs/navigation.md` records it as an addition with no
  TypeScript counterpart, outside the table of document-symbol differences,
  whose codes the conformance runner checks. The
  owner's doc comment states that it ports no TypeScript function and names
  the Vitest API it recognizes: `describe`, `it`, and `test` with the
  modifiers above, and the four hooks. It also names the version the
  modifier list was taken from, so that a later Vitest modifier is a known
  gap rather than a silent omission. That version is Vitest 3.2.4, the one
  remote-ai-manager installs. In its `@vitest/runner` types:
  - `ChainableTestAPI` has the keys `concurrent`, `sequential`, `only`,
    `skip`, `todo`, and `fails`, plus `each` and `for`.
  - `ChainableSuiteAPI` has the same keys with `shuffle` in place of `fails`,
    plus `each` and `for`.
  - `skipIf` and `runIf` sit on the wrapping types, `TestAPI` (through
    `ExtendedAPI`) and `SuiteAPI`, and each returns the chainable type.

  The owner walks the final tree with an explicit stack, such as
  `Ast::walk`'s, does not descend into computed property names, and finds
  each entry's parent by ancestry, with a stack of open entries. It then
  orders the entries by position (see "Containment and order"). It walks a
  chain in a loop, since a member chain can be as long as the file. Nothing
  recurses natively.
- **Input:** `navigation::file::navigate` is the input owner for
  `--outline FILE`: it reads, classifies, and parses. It answers from a
  `Catalog` and drops the tree first, so the test view shares its read,
  classify, and parse step and answers from the tree.
- **Names:** `names::source_text` plus line-break removal, which is the part
  of the outline's argument rendering that applies. `names::callback` itself
  keeps only string and template arguments, so it would drop `Widget.name`,
  which the view must show as written.
- **Library:** `navigation::test_outline_file(path)` returns a `TestOutline`
  with `file` and `entries`, or an `AnalysisError`. The entries are flat, in
  source order, each with `anchor`, `call`, `name`, and `depth`, as
  `Outline::entries` are, and they serialize with serde.
- **CLI:** `FileOperation::TestOutline` under `Target::File`, and a registry
  entry whose flags are `--limit`, `--json`, `--pretty`, and `--output` (its
  registered alias `-o` comes with it), besides the global `--help` and
  `--version`. Its
  renderer sits in `cli::navigation` beside the outline's and applies the cap
  through `Cap`, so the cut line, `display`, and `next` come from the one
  delivery owner. The list is `List::Entries` with its own `Bounded` (default
  200, summary `entries, in source order`), because `ENTRIES`'s summary names
  `--depth`, and help prints the summary as the operation's display limit.
  File answers reach delivery as `ProjectResponse` (`cli::query::Answer`), so
  the view's result is a `ProjectResponse::TestOutline` variant that no
  `ProjectRequest` produces. No project request, load, session, or service
  handler changes, and there is no `--project`.

The outline is unchanged and remains a port of TypeScript's document
symbols.

### Alternatives considered

- **A mode of `--outline` (`--outline --tests`).** Agents already reach for
  the outline, but the mode would need different defaults (every level, no
  locals), would have to reject `--depth` and `--project` or define them,
  and would make one operation answer two questions.
- **Change the outline's labels for test calls.** Departs from TypeScript's
  document symbols in the one operation whose contract is to port them, and
  still leaves the locals and the depth problem.
- **Any call chain rooted at `describe`, `it`, or `test`.** Simpler to state,
  but it reports fixture-object statements in real suites as tests.

## Examples

The sample `edge-cases.test.ts`, written `/abs/edge-cases.test.ts` here (real
output names its absolute path):

```ts
 1 import { describe, it, test, beforeEach, expect } from "vitest";
 2 class Widget {}
 3 describe(Widget.name, () => {
 4   beforeEach(() => {});
 5   it.each([1, 2])("adds %i", (n) => { expect(n).toBeTruthy(); });
 6   describe.each([["a"], ["b"]])("case %s", (x) => {
 7     it(`handles ${"x"}`, () => {});
 8   });
 9   it.todo("writes later");
10   it.skip("skipped one", () => {});
11   test("with options", { timeout: 1000 }, async () => {});
12   it.skipIf(process.env.CI)("conditional", () => {});
13   test.concurrent("parallel", async () => {});
14 });
15 test.describe("playwright suite", () => {
16   test("pw case", async ({ page }) => {
17     await test.step("first step", async () => {});
18   });
19 });
```

### The test view

`tsparse --test-outline /abs/edge-cases.test.ts` prints this and exits 0:

```text
/abs/edge-cases.test.ts
3:1 describe Widget.name
  4:3 beforeEach
  5:3 it.each() "adds %i"
  6:3 describe.each() "case %s"
    7:5 it `handles ${"x"}`
  9:3 it.todo "writes later"
  10:3 it.skip "skipped one"
  11:3 test "with options"
  12:3 it.skipIf() "conditional"
  13:3 test.concurrent "parallel"
16:3 test "pw case"
```

The import, `class Widget`, the `{ timeout: 1000 }` options object, and the
`expect(n)` line are not test calls. Neither is `test.describe(…)`, since
`describe` is not a modifier, nor `await test.step(…)`. Line 16's
`test("pw case", …)` is a test call outside any test call, so it is at the
top level.

With `--json`, the document is one line with no whitespace between tokens;
it is wrapped and indented here at entries:

```json
{"entries":[{"anchor":{"column":1,"line":3},"call":"describe","children":[
  {"anchor":{"column":3,"line":4},"call":"beforeEach","children":[]},
  {"anchor":{"column":3,"line":5},"call":"it.each()","children":[],"name":"\"adds %i\""},
  {"anchor":{"column":3,"line":6},"call":"describe.each()","children":[
    {"anchor":{"column":5,"line":7},"call":"it","children":[],"name":"`handles ${\"x\"}`"}],"name":"\"case %s\""},
  {"anchor":{"column":3,"line":9},"call":"it.todo","children":[],"name":"\"writes later\""},
  {"anchor":{"column":3,"line":10},"call":"it.skip","children":[],"name":"\"skipped one\""},
  {"anchor":{"column":3,"line":11},"call":"test","children":[],"name":"\"with options\""},
  {"anchor":{"column":3,"line":12},"call":"it.skipIf()","children":[],"name":"\"conditional\""},
  {"anchor":{"column":3,"line":13},"call":"test.concurrent","children":[],"name":"\"parallel\""}],"name":"Widget.name"},
 {"anchor":{"column":3,"line":16},"call":"test","children":[],"name":"\"pw case\""}],
 "file":"/abs/edge-cases.test.ts","operation":"test-outline","outcome":"ok"}
```

`tsparse --test-outline /abs/edge-cases.test.ts --limit 3` shows the first
three entries and the cut line:

```text
/abs/edge-cases.test.ts
3:1 describe Widget.name
  4:3 beforeEach
  5:3 it.each() "adds %i"
… 8 more entries; see all with: tsparse --test-outline /abs/edge-cases.test.ts --limit all, or write them to a file with -o FILE
```

The insertions below use `new-test.ts`, which holds the line
`it("new test", () => {});`, and `new-hook.ts`, which holds the line
`afterEach(() => {});`.

### The `it.skip` statement, 10:3

Today `--show --at 10:3` is a missing selection. With statement sites,
`tsparse --show --at 10:3 /abs/edge-cases.test.ts` prints this and exits 0:

```text
/abs/edge-cases.test.ts:10:3 statement it.skip("skipped one", () => { … })
10 | it.skip("skipped one", () => {});
```

With `--json`:

```json
{"declaration":{"anchor":{"column":3,"line":10},"kind":"statement","label":"it.skip(\"skipped one\", () => { … })","range":{"end":{"column":36,"line":10},"start":{"column":3,"line":10}}},"file":"/abs/edge-cases.test.ts","operation":"show","outcome":"found","source":"it.skip(\"skipped one\", () => {});"}
```

`tsparse --insert-after --at 10:3 /abs/edge-cases.test.ts --from new-test.ts`
previews this and exits 0. The edit document's `site` adds `"depth": 2`,
because the `describe() callback` entry at depth 1 contains the statement:

```text
/abs/edge-cases.test.ts:10:3: insert_after statement it.skip("skipped one", () => { … }): after the statement at 10:3-10:36
validated: syntax and edit boundaries; not type-checked
--- /abs/edge-cases.test.ts
+++ /abs/edge-cases.test.ts
@@ -8,6 +8,7 @@
   });
   it.todo("writes later");
   it.skip("skipped one", () => {});
+  it("new test", () => {});
   test("with options", { timeout: 1000 }, async () => {});
   it.skipIf(process.env.CI)("conditional", () => {});
   test.concurrent("parallel", async () => {});
preview only: save a plan with --plan PLAN to apply it
```

### The hook, 4:3

`tsparse --show --at 4:3 /abs/edge-cases.test.ts` prints this and exits 0:

```text
/abs/edge-cases.test.ts:4:3 statement beforeEach(() => { … })
4 | beforeEach(() => {});
```

`tsparse --insert-after --at 4:3 /abs/edge-cases.test.ts --from new-hook.ts`
previews this and exits 0:

```text
/abs/edge-cases.test.ts:4:3: insert_after statement beforeEach(() => { … }): after the statement at 4:3-4:24
validated: syntax and edit boundaries; not type-checked
--- /abs/edge-cases.test.ts
+++ /abs/edge-cases.test.ts
@@ -2,6 +2,7 @@
 class Widget {}
 describe(Widget.name, () => {
   beforeEach(() => {});
+  afterEach(() => {});
   it.each([1, 2])("adds %i", (n) => { expect(n).toBeTruthy(); });
   describe.each([["a"], ["b"]])("case %s", (x) => {
     it(`handles ${"x"}`, () => {});
preview only: save a plan with --plan PLAN to apply it
```

### The `it.each` callback's anchor, 5:30

The callback's anchor is its first token, the `(` of `(n)`, as projection
anchors every unnamed site. The position selects that entry, so `--show`
keeps today's answer exactly. `tsparse --show --at 5:30 /abs/edge-cases.test.ts`
prints this and exits 0:

```text
/abs/edge-cases.test.ts:5:30 function <function>
5 | (n) => { expect(n).toBeTruthy(); }
```

`tsparse --insert-after --at 5:30 /abs/edge-cases.test.ts --from new-test.ts`
is refused. It exits 1 and prints this on stderr:

```text
/abs/edge-cases.test.ts:5:30: edit 1: <function> cannot have statements inserted after it: this function is not a statement in a statement list (statements can be inserted only after a statement of a file, block, module block, or case clause); to insert after the enclosing statement, select it with --at 5:3
```

The innermost statement containing the arrow is line 5's expression
statement, an item of the `describe` callback's body. `--at 5:3`, on its
`it`, matches no entry and selects that statement alone. With `--json`, the
document is:

```json
{"effect":"none","error":{"edit":0,"file":"/abs/edge-cases.test.ts","kind":"UnsupportedSite","message":"<function> cannot have statements inserted after it: this function is not a statement in a statement list (statements can be inserted only after a statement of a file, block, module block, or case clause); to insert after the enclosing statement, select it with --at 5:3","position":{"column":30,"line":5},"proposed":null},"operation":"edit","outcome":"rejected"}
```

Following the remedy,
`tsparse --insert-after --at 5:3 /abs/edge-cases.test.ts --from new-test.ts`
previews this and exits 0:

```text
/abs/edge-cases.test.ts:5:3: insert_after statement it.each([1, 2])("adds %i", (n) => { … }): after the statement at 5:3-5:66
validated: syntax and edit boundaries; not type-checked
--- /abs/edge-cases.test.ts
+++ /abs/edge-cases.test.ts
@@ -3,6 +3,7 @@
 describe(Widget.name, () => {
   beforeEach(() => {});
   it.each([1, 2])("adds %i", (n) => { expect(n).toBeTruthy(); });
+  it("new test", () => {});
   describe.each([["a"], ["b"]])("case %s", (x) => {
     it(`handles ${"x"}`, () => {});
   });
preview only: save a plan with --plan PLAN to apply it
```

## Verification

Each rule's variants get a test that asserts the final query or edit result.
The selection tests cover:

- each statement kind's first token;
- a position inside a first token and one at its end;
- an unbraced body, a labeled statement, a decorated class, and the `export`
  forms;
- a class static block;
- the precedence cases;
- every position that selects an entry today, before and after the change;
- unchanged record ordinals: the default `--outline`, and project-mode
  `--show --name` and name-selected project edits in a release build, give
  today's results on a file with many statements before its declarations.

The edit tests cover:

- insertion and body replacement at statement sites, single and in a batch;
- saving and applying a plan;
- `--project` through the service and with `--direct`, byte-identical;
- the remedy, followed, and each case that prints none.

For the test view:

- each root, modifier, and form that is not a chain, including a tagged
  template table;
- a test call inside a computed property name, which is not listed, and
  every listed position followed with `--show --at`, which selects that
  test's statement;
- every flag in the operation's accepted set (`--limit`, `--json`, `--pretty`
  with `--json`, `--output` and its alias `-o`, and the global `--help` and
  `--version` with their aliases `-h` and `-V`), which is accepted, and every
  flag outside that set, such as `--project`, `--depth`, or `--at`, which is
  refused as a usage error;
- names with line breaks;
- containment through transparent calls and helpers;
- the cap just under, at, and over 200, with and without `-o`, with the
  printed `next` command followed;
- an empty file, and each failure.

## Documentation and agent surfaces

The same change updates:

- `docs/navigation.md`: selection by position, the missing-selection
  sentence, and the test view as a deliberate addition;
- `docs/editing.md`: E2, with statement sites and the remedy;
- `plugins/tsparse/skills/tsparse/SKILL.md`, and
  `plugins/tsparse/.claude-plugin/plugin.json`, whose version or scope the
  new operation changes;
- `SHARED_AGENTS_BLOCK.md`, with its one-line-per-operation list;
- `COMMANDS.md` and `docs/cli-reference.md`, regenerated from the registry, as
  `AGENTS.md` requires.

## Not in this design

- Playwright's `test.describe` and `test.step`, and `suite`: not used in the
  motivating repository.
- Selecting a test by its name: positions from the view are the selector.
- A pointer from the default outline to the test view.
- Guarding against a non-test function named `describe`, `it`, or `test`
  beyond the modifier list.
- `--project` or service support for the test view.
- Any change to the default `--outline` output.
