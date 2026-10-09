# Source-preserving editing

`tsparse --replace-body` replaces the body of one declaration, and
`tsparse --insert-after` inserts statements after one, selecting it as
[`--show`](navigation.md) does. A request prints the exact change as a unified
diff and writes nothing; `--plan` saves the prepared batch, and
`tsparse --apply PLAN` writes exactly those bytes. Every edit is prepared
against the original files, composed with the others for its file, parsed,
and checked so that the syntax it does not replace keeps its meaning and
comment directives and JSDoc keep governing what they governed. The Rust
library exposes the same operations as `typescript_parser::editing` (see
[Library](#library)). The design is recorded in
[editing-design.md](editing-design.md), with the decisions made while
implementing it in [editing-plan.md](editing-plan.md); this document describes
the implemented behavior.

Validation is syntax and edit-boundary validation. A prepared edit parses and
keeps every protected relationship; it is not type-checked.

## Commands

```sh
# Preview one edit; body.ts holds the complete new body, { ... }.
tsparse --replace-body --name createRunHost.startEligible --from body.ts src/runs/host.ts

# Preview and save a plan, in a project, through its service.
tsparse --insert-after --project tsconfig.json --name projectRunHalts \
  --from helpers.ts --plan .cc/temp/helpers.plan.json

# Several edits, prepared together against the original files.
tsparse --edit-batch edits.json --project tsconfig.json --plan .cc/temp/change.plan.json

# Apply a saved plan, without selecting anything again.
tsparse --apply .cc/temp/change.plan.json
```

| Operation | Selection | Inputs | Options |
| --- | --- | --- | --- |
| `--replace-body` | `--name NAME [FILE]` or `--at LINE:COLUMN FILE` / `--at FILE:LINE:COLUMN`, as `--show` takes them; FILE is required without `--project` | `--from FRAGMENT`, a file or `-` for stdin | `--project TSCONFIG [--direct]`, `--plan PLAN`, `--json [--pretty]`, `-o` |
| `--insert-after` | as `--replace-body` | as `--replace-body` | as `--replace-body` |
| `--edit-batch EDITS` | in the batch file | the batch file and the fragments it names | as `--replace-body` |
| `--apply PLAN` | none: the plan records its files and sites | the plan | `--json [--pretty]`, `-o` |

A batch file is `{"edits": [...]}`, each edit
`{"operation": "replace_body" | "insert_after", "name": NAME | "at": AT, "file": FILE, "fragment": TEXT | "from": PATH}`
with exactly one of `name` and `at` and exactly one of `fragment` and `from`;
`at` is `LINE:COLUMN` with `file`, or `FILE:LINE:COLUMN`. Unknown fields are
errors. Paths in arguments and batch files are relative to the working
directory; plans record absolute paths, so a plan applies from anywhere.

The request parser rejects, as usage errors (exit 2): two operations;
`--from` or `--plan` without an edit operation; a missing `--from`; both or
neither of `--at` and `--name`; `--name` without FILE or `--project`; FILE `-`;
`--direct` without `--project`; `--apply` with a selection, file, or project;
`--edit-batch` with a selection, `--from`, or FILE; `--check`, `--tsx`,
`--depth`; `--pretty` without `--json`; and a value option given twice.
An editing command writes `-o` and `--plan` only as new files: a path that
exists, even as a symbolic link to nothing, is a usage error before anything
is prepared or applied, so no outcome's document or plan can replace a file it
read, selected, or edits. Nor may either create a file the command names that
does not exist, such as a deleted target: a FILE or batch `file`, the
project's configuration, the plan being applied, or a name the plan records
(each file's names, the file they resolved to, and the project's
configuration), which applying reads from a plan's text even when the plan
cannot be loaded. `-o` and `--plan` may not name one new file either. When
such a name does not exist, the file system decides which spellings name one
file, through links, case, and Unicode forms alike: `tsparse` creates the
output for the question, looks for it under each such name, and removes it
again, first freeing it of what its directory gave it if that denies the
removal; an output it still cannot remove is reported as left there, empty.
A plan whose text cannot be read, or does not decode as a plan, names files
no one can know, so with `-o` its load failure goes to stderr, exit 2, and
nothing is written.

## Body replacement (E1)

The selected site's callable is the site itself when it is a function
declaration or expression, arrow function, method, constructor, get or set
accessor, or, for a variable, property, binding element, or parameter
property, its initializer when that is directly a function or arrow
function, inside any parentheses. Nothing else is unwrapped: a call,
condition, `as`, or `satisfies` around a function leaves the variable without
a body to replace, and the function inside is selected by its own position. A
callable without a body (an overload, abstract member, or signature), an
ambient one, and every other kind of site, such as a class, are
`UnsupportedSite`.

The fragment is the complete new body: a block `{ ... }`, or, for an arrow
function only, an expression. It replaces the body's tokens exactly: the
name, modifiers, type parameters, parameters, return type, and everything
around the callable stay byte for byte, and a comment before the body is
kept. A `return` is the caller's to write. An arrow may change between a
block and an expression; `{ a: 1 }` is a block, as TypeScript parses it, and an
object needs `({ a: 1 })`. Comments at the fragment's edges are allowed when
the checks below hold.

## Statement insertion (E2)

The selected site designates a statement, as `--show` excerpts it: a binding
designates its whole variable statement, a specifier its whole import or
export declaration, and any other site itself. That statement must be an item
of a statement list: a file, block (including a function body or class static
block), namespace body, or `case` or `default` clause. A binding in a loop
header, a `catch` variable, a parameter property, a class or object member,
a callback, and a declaration that is the unbraced body of a control-flow or
labeled statement have no such list and are `UnsupportedSite`; selection never
climbs to an enclosing statement instead.

The fragment is one or more complete statements; a fragment of only
whitespace and comments inserts none and is rejected. It goes after the
comments that trail the anchor on its line, which stay with the anchor, and
before the comments, JSDoc, and directives on later lines, which stay with
what follows. It starts a line of its own at the anchor's indentation; code
that followed the anchor on its line moves to a line of its own after it. A
blank line after the anchor, as between declarations, separates the
insertion from the anchor too. The preview reports the anchor statement's
range, including when several bindings share it.

A line break alone does not always end a statement. A `;` is inserted after
the anchor when it ends without one, is not closed by its own body, and the
fragment starts with a token that would continue its expression: `(`, `[`, a
template, `+`, `-` (not `++` or `--`), `/`, or `<`. At the fragment's end,
when the following statement starts with such a token, a `;` goes right after
the fragment's last token, before any comment after it, whenever the
fragment's last statement can take one: an expression, a declaration without
a body, a `do` loop, the unbraced body of an `if`, loop, or label, and any
other statement that ends with an optional `;`, which is harmless where it is
not needed. Both are shown in the diff. A fragment that ends with an incomplete statement,
such as `if (x)`, is rejected.

## What every edit keeps

Bytes outside the replaced spans and insertion seams stay identical,
including a byte order mark, a `#!` line, comments, line breaks, and the final
newline. Inserted lines take the seam's indentation and line-break style (the
line break that ends the seam's line, else the file's first, else LF): every
line of the fragment moves from the fragment's common indentation to the
seam's, and blank lines lose their spaces. A line that begins inside a string,
template, regular expression, JSX text, or comment keeps its bytes and the
line break before it.

Preparation parses the proposed file in its own file name (`.tsx` parses JSX)
and rejects it unless:

- **Syntax.** It parses (`Syntax`, located in the proposed text and the
  edit), within the parser's grammar budget (`ResourceLimit`).
- **Boundaries.** Every node outside the opened slots corresponds to the
  original by kind, fields, exact values, flags, and token positions through
  the change map (`Boundary`). A body fills exactly its slot; inserted
  statements lie in their gap; preserved statements stay separate, in order,
  in their list. A deprecation tag that would now apply to a preserved
  declaration changes its flags and is rejected. The file's dynamic-import
  and `import.meta` summaries may change. Adding `export {}` may make a script
  a module, and is allowed unless it reparses preserved code, such as
  `await(foo)`, which becomes an `await` expression.
- **Comment relationships.** Every preserved line is governed by the same
  `@ts-ignore` or `@ts-expect-error` directive as before, or by none, under
  TypeScript's `getDiagnosticsWithPrecedingDirectives` reach; a directive in
  a fragment may govern only new code, and may not take over a line another
  directive governed (TypeScript would report the old `@ts-expect-error` as
  unused). The first preserved syntax after each change collects the same
  JSDoc comments as before, under `GetJSDocCommentRanges` (`Attachment`).
  An ordinary trailing comment is harmless.

## Batches

All edits of a request are selected in the original files, never in the
results of other edits, and every edit of a file is composed and validated
together, so the checks cover the complete result. A file named two ways, such
as through a symbolic link, is one file; names that would parse it differently
(`.ts` and `.tsx`) are refused. Edits conflict (`Conflict`) when their changes
overlap or nest, repeat, insert at the same place, or insert at the boundary
at the start of a body another edit replaces, since each would need an
ordering rule; combine such edits into one fragment. An insertion after a
statement whose body another edit replaces follows that body. A batch is prepared completely or
rejected; no subset is ever offered. A batch that changes nothing is
`no_change`.

## Plans and applying

A plan is an inspectable JSON file recording, per file, the path analysis
parsed it by (as named, or the canonical path a project selected), any other
names the request used for it, the regular file they resolve to, its device,
inode, mode, owner, group, and flags,
whether it begins with a byte order mark, its original and proposed text, and
each edit's operation, selected site with its anchor, fragment, and changes.
Loading a plan reads no current file, fragment, or project: it prepares the
recorded edits again, from their anchors, against the recorded originals, and
accepts the plan only if that gives exactly the recorded proposed text. Its
file names must be ones preparation would edit. A plan from another build of
`tsparse` is refused; prepare it again. A plan
changes nothing until applied.

Applying is supported on macOS and Linux. It checks every file: each name
still resolves to the same regular file with one link, the same device, inode,
mode, owner, group, and flags, and exactly the original bytes; timestamps are
not identity. Then it stages every replacement beside its file, with the
original's permissions, owner, group, extended attributes, and access control
lists, by one explicit rule: the new file starts free of what its directory
gave it that could deny its owner the writes that follow (on macOS an
inherited list, on Linux an owner entry without write), each attribute's
value is set, each attribute the new file has beyond the original's is
removed (such as the access list a directory's default list gives it on
Linux), the original's list, or none, is set last since it can deny the
writes before it, and everything is compared both ways after the final mode
change, so metadata that cannot be kept fails staging instead of being
dropped, rewritten, or added.
`com.apple.provenance`, which macOS gives every file a process writes and
which cannot be removed or set, is the one attribute exempt. A file with macOS
flags (`chflags`, such as `hidden` or `uchg`) is not edited; Linux file
attributes (`chattr`) are not inspected. Staging links the original under a
backup name; a name it did not create is never removed. It checks every file
again, replaces them one rename each in path order, and removes the backups.
If a replacement fails, it restores the files it replaced, unless a file is no
longer the one it installed: another writer's bytes are never overwritten.
Applying a plan again finds its files changed.

Keep other writers away from the files while applying. Readers may see some
files replaced before others; a crash may leave temporary or backup files
(`.NAME.tsparse-PID-N.new` and `.orig`) and some files replaced.

| Situation | `effect` | Exit |
| --- | --- | --- |
| The plan changes nothing | `no_change` | 0 |
| Every file replaced | `applied`, with any file that could not be removed in `leftovers` | 0 |
| A file changed since preparing | `unchanged` | 1 |
| A file could not be read, staging failed, or the first replacement failed | `unchanged` | 2 |
| A later replacement failed and every replaced file was restored | `rolled_back` | 2 |
| A replaced file could not be restored | `recovery_required`, with each file's state and where its original is kept | 2 |

When an apply's result cannot be written, stderr gives the write failure and,
for every effect but `no_change`, then the outcome's text (`applied: …`, the
restored files, each file needing recovery and where its original is kept,
why nothing changed, or why the plan could not be loaded), with any
leftovers; the command exits 2. When a preview's result cannot be written
after its plan was saved, stderr gives the write failure and then that the
plan was saved, and where.

## Project mode

With `--project`, the project's service (or this process, with `--direct`)
selects each edit over one generation of the project: the searched file a
FILE names, the one file whose declaration a name alone selects, or the
ambiguous or missing outcome `--show` gives. It returns each edited file's
captured text and parses nothing. This process then reads each file, requires
the text the project selected in, which refuses a file changed since then
(`Stale`), and prepares the edits once, as for named files. Only this process
writes, so a lost service reply never leaves an unknown write. Applying does
not reload the project; the next project query refreshes as usual.

## Output

Text: per edit, its selected site as `FILE:LINE:COLUMN: operation kind label
(selector)`, where it changes the file (`body L:C-L:C`, or `after the statement
at L:C-L:C`), then `validated: syntax and edit boundaries; not type-checked`
and a unified diff of each changed file (a `# line breaks:` line names breaks
other than LF), and the plan's path or `preview only`. An apply prints
`applied: FILES changed`. Failures are one diagnostic on stderr.

JSON (`--json`): one document with `operation` (`edit` or `apply`), `outcome`,
and `effect`. A preview has `outcome` `preview` or `no_change`, `effect`
`none`, `validation`, `plan`, `project` in project mode, and `files`, each with
`file`, `path`, `changed`, `edits` (index, operation, site, fragment, and
changes with byte spans, positions, and text), and `diff`. Ambiguous and
missing selections are `show`'s documents with `operation` `edit` and the
`edit` index; an analysis failure is the navigation error document with
`operation` `edit` and `effect` `none`; a rejection has `outcome` `rejected` and `error` with `kind`,
`message`, `edit`, `file`, `position`, and `proposed`. An apply has `files`
with each file's `state` (`unchanged`, `replaced`, `restored`, or
`recovery_required` with what it holds and where its original is), `leftovers`,
and, on failure, `error` with `kind` `Stale` or `Io`.

| Outcome | Exit |
| --- | --- |
| Preview, `no_change`, applied | 0 |
| Ambiguous or missing selection, rejected edit, plan from another build or that no longer prepares to its text, stale files | 1 |
| Usage error, unreadable input or plan, failed write, rollback, recovery, unreachable service | 2 |

Analysis failures keep their [navigation](navigation.md#exit-codes-and-channels)
codes.

## Library

```rust
use typescript_parser::editing::{self, Edit, EditRequest, Operation, Outcome};
use typescript_parser::navigation::ShowQuery;

let request = EditRequest { edits: vec![Edit {
    operation: Operation::ReplaceBody,
    target: ShowQuery::Name { name: "start".into(), file: Some("src/host.ts".into()) },
    fragment: "{ return true; }".into(),
}] };
if let Outcome::Ready(batch) = editing::prepare(&request)? {
    print!("{}", batch.files()[0].diff());
    batch.save(std::path::Path::new("change.plan.json"))?;
}
```

`editing::prepare` prepares edits of named files; in a project,
`ProjectSession::execute(ProjectRequest::Edit(request))` answers a
`Selected`, and `editing::bind` prepares its selection. Both give an
`Outcome<PreparedBatch>`: `Ready`, `Unselected` (with `show`'s outcome), or
`Rejected` (a `Rejection` with its `RejectionKind`); inputs that cannot be
analyzed are `AnalysisError`s. A `PreparedBatch` exposes each file's original
and proposed text and bytes, edits, and diff, and is constructed only by
preparation: `PreparedBatch::save` writes a plan, `PreparedBatch::load` prepares
one again, and `PreparedBatch::apply` applies it, returning an `Applied` with
its `Effect` and per-file `FileState`s.

## Verification

`tests/editing.rs` covers sites, layout, separators, comment relationships,
batches, and limits; `tests/editing_apply.rs` the filesystem effects, with
failures injected at each step of applying; `tests/editing_plans.rs` plans;
`tests/editing_project.rs` project selection and binding; and
`tests/editing_cli.rs` the command contract, including service and direct
agreement. `tools/editing-flows.mjs` runs preview, plan, apply, and follow-up
queries on a disposable project copy; see
[validation](validation.md#editing-on-command-center).
