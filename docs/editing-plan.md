# Source preserving editing: implementation plan

Scope: E, E1, E2 as defined by the [editing design](editing-design.md), on the
navigation delivery at `ac24bc5`. This plan fixes the order, owners, and
evidence. Rust layout below names the knowledge boundaries the design draws;
helpers inside them remain implementation choices.

## Working method

Behavior-level red–green–refactor. A new behavior starts with a failing test of
a public operation (`editing::*`, `Project::execute`, or the CLI), run through
the registered command scoped to its file:

```sh
cctl validate run test --queue-if-busy -- tests/editing.rs
```

Expectations for language behavior come from TypeScript 7.0.2 (the pinned
compilers under `.cache/compiler-oracles`); product policy (layout, seams,
plan format, exit codes) comes from the design. Pure wiring and mechanical
moves may skip test-first, with the reason stated. Adversarial review rounds
(Opus 5.5 and Codex, both extra-high effort, both using the
ousterhout-software-design skill) follow steps 3, 5, and 6.

## Owners

| Design boundary | Rust home | Hides |
| --- | --- | --- |
| Complete editing operations | `editing` (`mod.rs`) | Request, selection, and outcome types, the one batch construction, binding, save/load, preview, apply entry points. |
| Selected syntax | `navigation::catalog::Syntax` over `projection` records | Which syntax node a selected site is and which statement it designates; lends both only while the tree lives. |
| Batch preparation (per file) | `editing::prepare` | One original parse, selection at a site, policy dispatch, composition, one or two candidate parses, validation. |
| Operation policies | `editing::policy` | E1 callable/body eligibility; E2 statement-list membership; the slot each opens. |
| Trivia and seams | `editing::seam` | Seam placement after same-line trailing comments, statement terminators, indentation and line-ending layout, and the attachment record/verify (directive reach, JSDoc collection). |
| Source change composition | `editing::compose` | Original-coordinate changes, conflicts, ordering, the change map. |
| Shared syntax correspondence | `editing::compare` | Field-by-field comparison of protected syntax through the change map with the design's named treatments. |
| Captured file identity and replacement | `editing::file` | Path/referent identity, metadata, raw image, temporary and backup resources, install and restore. |
| File batch application | `editing::apply` | Staging, recheck, ordered install, rollback, cleanup, and the reported filesystem effect. |
| Preview | `editing::diff` | Unified diff from the exact images. |
| Project selection | `project::editing` | Each edit's file through the snapshot's indexes, positions checked against captured text, and the captured text returned; no parse. |
| CLI | `cli::request`, `cli::editing`, `cli::delivery` | Option validity, rendering, channels, exit codes. |

Existing owners extended rather than copied: `lexer` (comment scanning around
literals; the collection half of `GetJSDocCommentRanges`), `syntax` (its kind
half, `trailing_jsdoc`; `SkipParentheses`), `pragmas` (the governing directive
of each line from `getDiagnosticsWithPrecedingDirectives`, moved out of
`project::program::diagnostics`), `analysis` (`InputError::unparsable`, the one
classification of a failed parse of analyzed text, with `ResourceLimit` for an
exhausted grammar budget), `source` (raw image prefix), and `input` (the
absence rule, lifted from `project::program::disk`).

## Sequence

1. **Selected syntax.** Projection records carry their syntax node and
   designated statement; `SkipParentheses` gets one owner. Navigation tests stay
   green (mechanical; no new behavior).
2. **E1 preparation in text.** `editing` preparation over a captured text:
   selection, E1 policy, composition, validation with structural comparison.
   Tests assert proposed text or typed rejection for every callable form,
   wrappers, bodyless sites, multi-binding statements, slot escapes.
3. **E2, seams, layout, attachment.** Statement-list membership, seam placement,
   terminators at both ends, indentation/line-ending layout with literal and
   comment interiors protected, directive reach and JSDoc collection checked on
   the final source. Review round 1.
4. **Files, batches, plans, apply.** Capture, binding, conflicts across edits,
   plan save/load by replay, application with staging, recheck, rollback, and
   fault injection at the replacement boundary.
5. **CLI and project/service.** `--replace-body`, `--insert-after`,
   `--edit-batch`, `--apply`; `ProjectRequest::Edit` through
   `ProjectSession::execute` and the service; text and JSON delivery. Review
   round 2.
6. **Documentation and real-project proof.** Contract documentation, help text,
   a disposable Command Center copy (`show → preview → apply → show/references`),
   timings. Final review round.

## Implementation outcome

### Delivered by step

| Step | Commits | Delivered |
| --- | --- | --- |
| 1–3 | `e3760ff`, `21b5243` | Selected syntax lent by `navigation::Syntax`; shared owners for comment scanning, JSDoc collection, directive coverage, and `SkipParentheses`; preparation with both operations, seams, layout, composition, comparison, and attachment checks. |
| 4 | `01b231e`, `443775b`, `0445eea` | Capture and identity, staged application with hard-link backups, rollback and recovery reporting, fault injection; plans saved as new files and loaded by replay; Linux verification in a container. |
| 5 | `1650a96`, `993dadc` | `ProjectRequest::Edit` selecting over one generation; local binding; `--replace-body`, `--insert-after`, `--edit-batch`, `--apply`; text and JSON delivery. |
| Round 1 | `01a5f3c`, `3466a08` | The review fixes listed under [Reviews](#reviews). |
| 6 | `79eb6e6`, `4176c28` | Blank-line separation, `tools/editing-flows.mjs`, the Command Center proof, `docs/editing.md`, README, and validation records; output aliasing checked before preparing; a bounded preview diff. |

### Decisions made during implementation

- **One construction path is preparation itself.** A file's proposed text is
  only ever produced by `editing::prepare::prepare` over a capture, through
  `editing::construct`. File mode prepares from its capture. Project mode
  selects in the project (no parse) and binds the selection: the invoking
  process captures each file, requires the selected text, and prepares once.
  Loading a plan prepares the recorded edits again from each site's anchor
  against the recorded original bytes and requires the recorded proposed
  text, which is only compared, never trusted. Preparation rejects a site
  whose anchor would not select it alone, so replay is deterministic.
- **Selected syntax.** Projection records carry their node and designated
  statement (`Record::node`, `Record::designation`), and a record's excerpt is
  its designation's token span. Only `navigation::Syntax`, which holds the
  tree, lends them, so selection, candidates, and excerpts stay `show`'s.
- **Separators.** A `;` goes after the anchor statement when it ends without
  one, is not closed by its own body, and the fragment's first token would
  continue an expression across a line break: `(`, `[`, template, `+`, `-`
  (not `++` or `--`), `/`, `<`, the set the pinned 6.0.3 syntax API confirms.
  At the fragment's end, where the following statement starts with such a
  token, a probe `;` on its own line in the draft shows whether one is
  needed: unless the probe forms an empty statement (of its own, or as an
  incomplete statement's body), it terminated something, and a `;` goes right
  after the last token before it, skipping comments. That covers `do` loops,
  bodiless declarations, and unbraced bodies, whose final token their own
  last child does not end. Trivia-only fragments get no separators, so they
  insert no statement and are rejected.
- **Layout.** The draft parse, needed only for multi-line fragments or a
  probe, supplies literal tokens and comments, which protect line starts
  (from a literal's first byte, which a JSX text line break can be). When
  settling changes nothing the draft is final, so a file parses at most three
  times. A single-line fragment takes the seam's indentation as a multi-line
  one does.
- **Comparison end positions.** A list a gap opens and a zero-width node such
  as the end-of-file token extend past an insertion at their end; a node that
  ended with a replaced body ends with its fill; a node that ends with its
  list's last item, such as a case clause, ends with the last statement
  inserted at the list's end; a statement that absorbs an inserted terminator
  is an explicit extension, which its seam reports.
- **Attachment by relationship.** Each preserved line keeps the same governing
  directive (`DirectiveCoverage::governing`), and the first preserved token
  after each change collects the same JSDoc comments, compared with itself.
- **Apply.** The original is hard-linked to a backup name beside it while the
  proposed bytes are staged in a temporary file with the original's mode,
  owner, group, extended attributes, and access control lists, by one
  explicit rule on both systems: the new file first freed of what its
  directory gave it that could deny its owner writes or its removal
  (`unrestrict`, which output checking shares), each
  value set, extras removed, the original's access control list (on Linux,
  its attributes) set last since it can deny the owner the writes before
  it, and everything compared both ways after the
  final mode change, with `com.apple.provenance` the one system-managed
  exemption; macOS flags are refused. Installation is one rename, and restoration renames the backup
  back after checking that the file is still the one installed. Resources
  belong to the replacement's state from their creation, so a failure inside
  staging releases or reports them like any cleanup, and a name staging did
  not create is never removed. The batch effect is derived from the per-file
  states. A file named several ways keeps every name, each checked again.
- **Outputs.** An editing command writes `-o` and `--plan` only as new
  files, refusing any existing path, a dangling symbolic link included; nor
  may they create a name the command reserves (the request's files, the
  configuration, the plan, and the names the plan records:
  `PreparedBatch::names`, or `LoadError::names` when a plan that decodes
  cannot be loaded, `None` when it cannot be read or decoded, which
  reserves every output), or one another. `editing::would_create` asks the
  file system which absent name an output would create, by creating it,
  looking for it under each such name, and removing it, rather than
  repeating its rules for links, case, and Unicode forms; it creates
  nothing when every name exists, and reports an output it cannot remove.
  Delivery
  tells a delivered non-zero outcome from a failed write, so an apply whose
  result cannot be written still reports its outcome, its leftovers
  included, and a preview's says its plan was saved.
- **Conflicts.** An insertion at the end of a replaced body follows it.

### Reviews

- **Round 1** (steps 1–3 at `21b5243`; Codex `5198edc0`, Opus `41e2db17`;
  reports in `.cc/temp/editing/reviews/round-1-*.md`). Accepted and fixed:
  directive retargeting through a still-covered line (both); JSX text
  starting with a line break rebased (both); separators from substring tests
  rejecting `'https://…'`, `{}`, and trailing comments, and generating a
  statement for trivia-only fragments (both); budget exhaustion in the
  original reported as `Syntax` (both; now `ResourceLimit` everywhere through
  one owner, which changes the kind navigation and project loading report);
  JSDoc compared across change regions instead of per preserved token; case
  and default clauses rejecting insertions after their last statement; `++`
  and `--` treated as continuations; single-line indentation; rejections not
  naming their edit; `prepare` interpreting seam internals; the JSDoc kind
  list outside its owner (Opus). Accepted as a design revision: the service
  selects and the invoking process prepares once (Opus), replacing the
  validated text proposal that binding prepared again.
- **Round 2** (steps 4–6 and the round-1 fixes at `4176c28`; Codex
  `c8bd3a21`, Opus `889a7a57`; reports in
  `.cc/temp/editing/reviews/round-2-*.md`). Accepted and fixed: outputs
  checked only by spelling and only on success, so a hard link, a project
  target, the configuration, or a plan that failed to load could be
  overwritten by a document (both); an apply whose result could not be
  written reported recovery as applied (both); attributes compared one way,
  macOS ACLs not compared, and macOS flags dropped (both); staging deleting
  a name it never created and losing partial resources (both); a retargeted
  second name of a deduplicated file not checked (Codex); terminators placed
  inside `do … while (…)` and bodiless declarations, and complete unbraced
  bodies rejected (both); plans bypassing capture's name admission (both);
  the flows tool not actually running the native compiler (Codex); a
  first-install failure reported as rolled back (Opus); analysis-failure
  documents without `effect` (Codex); the budget test not reaching the draft
  path (Codex); small decisions outside their owners (Opus). Accepted as a
  design revision: an insertion at a replaced body's end follows it (Opus).
- **Round 3** (confirmation at `4582a58`; Codex `23d092cd`, Opus
  `eb831a29`; reports in `.cc/temp/editing/reviews/round-3-*.md`). Opus
  judged the delivery complete with minor findings; Codex found output
  protection still incomplete. Accepted and fixed: outputs checked against
  files learned only on success, missing earlier-selected targets, sources
  that failed to parse, extended configurations, stale recorded aliases, and
  dangling symbolic links (Codex; Opus for the link), resolved by the design
  revision that results, like plans, are new files; an unchanged apply losing
  its leftovers when its result could not be written (both); macOS staging
  delegated to `fcopyfile`, which rewrote quarantine values and merged
  inherited list entries (Opus); the elision cursor's own `assertClause`
  rule (both); untested staging-name collisions and recorded-alias admission
  (Opus); the separator rule documented too narrowly (Opus).
- **Round 4** (confirmation at `aa06a54`; reports in
  `.cc/temp/editing/reviews/round-4-*.md`). Opus judged the delivery
  complete with three minor findings; Codex found one minor regression.
  Accepted and fixed: a result could recreate an absent file the command
  names, such as a stale plan's deleted target, under spellings differing by
  case, links, `.` and `..` (Codex), now reserved, the plan's names even
  when it cannot be loaded; macOS staging
  set the access control list before the attributes it could deny (Opus;
  Linux's list attributes sorted first too, now also last); a preview whose
  result could not be written did not say its plan was saved, and the
  contract claimed every undelivered apply repeats its outcome (Opus);
  `editing::same_file` answered a question no caller asks, now
  `same_new_name` for the one it does (Opus).
- **Round 5** (confirmation at `70fb7b8`; reports in
  `.cc/temp/editing/reviews/round-5-*.md`). Opus judged the delivery
  complete once the plan's configuration is reserved; Codex found four
  minor gaps. Accepted and fixed: a plan's recorded configuration not
  reserved on apply, the two lists of a plan's names (loaded and from its
  text) each missing it (Opus), now one list a test pins against the other;
  names the file system takes for one but `Case` folding does not, such as
  Unicode forms and `ß`/`ss` on APFS (Codex; Opus asked only for exact
  wording), resolved by a design revision that asks the file system
  (`would_create`), which also stops refusing other cases on case-sensitive
  volumes; one malformed field discarding every name of an unloadable plan,
  and unreadable names treated as none (Codex), now read field by field,
  with unknown names reserving every output; a list the staging file
  inherits denying the attribute writes before the original's list (both;
  a macOS regression from round 4, and an older Linux refusal for ordinary
  users), now cleared first; a plan-load failure lost when its result could
  not be written (Codex); missing reservation rows and an unasserted Linux
  attribute (Opus).
- **Round 6** (confirmation at `36954b8`; reports in
  `.cc/temp/editing/reviews/round-6-*.md`). Both confirmed the round-5
  fixes; Opus judged the delivery complete but for its first finding.
  Accepted and fixed: the output created to check it left behind, once as
  an empty deleted target, in a directory whose new files cannot be
  removed, and reported only as a failed comparison (both), now created
  only when a reserved name is absent, freed by `unrestrict` when its
  removal is denied, and reported as left otherwise, with the failing name
  given; the unknown-names refusal ran after that check (Codex); off Unix,
  every name compared equal, so every output was refused (Opus), now
  decided by existence, which needs no file identity; duplicate and
  wrong-shaped fields losing names (both), resolved by knowing a plan's
  names only from a strict decode; the user document naming only text that
  is not JSON (Opus).
- **Round 7** (closing confirmation at `eb91bf2`; reports in
  `.cc/temp/editing/reviews/round-7-*.md`). Both judged every round-6
  finding resolved and the delivery complete; Codex found no defect. Accepted:
  regression tests for an output left in an append-only directory and a
  reserved name that cannot be looked up, whose reports had no maintained
  test, and the design's claim that ordinary commands create nothing to
  check, which a new `-o` with a new `--plan` does (Opus).

### Change scenarios at closeout

The design's four ordinary changes, checked against the code:

- **A new callable form for E1** changes `policy::CALLABLES` or
  `policy::callable`, and `Opening::accepts` if its body differs; selection,
  composition, comparison, and application are untouched.
- **Member insertion** adds an `Opening` variant for a `members` list and a
  seam function for its separators; `compare::Slot::Gap` already compares any
  list field, and `file` and `apply` know nothing of syntax.
- **A lexical class change** belongs to `lexer::line_break`/`whitespace` or
  `source::line_breaks`, which layout, separators, and attachment consume;
  indentation's spaces and tabs and JSDoc's CR/LF rule are deliberately
  narrower and documented where they apply.
- **Replacement mechanics** change `file::Replacement`'s states and
  operations; `apply` uses only stage, recheck, install, restore, and release.

### How the design held up

The language core (selection, composition with slots, seams, attachment by
relationship, replay from anchors) took its final form by review round 3 and
needed no change after. Rounds 3 to 7 all concerned one boundary: what a
command may write besides its targets. Each round there found a case a
re-implemented rule missed, first files learned only on success, then case
folding against Unicode forms, then a lenient reader against duplicate
fields. Each was closed by handing the decision to the owner that already
makes it: outputs only as new files (`create_new`), the file system for
which spellings name one file (`would_create`), and the strict decoder for
which names a plan records. Ask the system that decides rather than predict
it.
