# Operating envelope

`tsparse` is a local CLI and a per-project background service that one author runs on their own machines, macOS and Linux, almost always through coding agents (Claude Code, Codex) that read its text or JSON output. Several agents run at once in separate git worktrees, each with its own service, and they edit files between queries. A failure costs the author's time; edited source lives in git. Nearest archetype: single-operator tool, daemon variant, driven by agents. It departs from the archetype in scale: it must answer quickly and within memory on very large TypeScript projects, so memory compaction, parallel freshness validation, and cross-generation reuse are inside the envelope.

| Dimension | Condition |
|---|---|
| Users | The author, through coding agents acting on the author's behalf. The author operates it at a terminal and is the only reader of its errors. A misbehavior affects only the author's checkouts. |
| Trust | Callers are trusted local processes of the same user. Everything parsed (sources, `tsconfig.json`, `package.json`, `node_modules`, saved plans, batch files) is hostile data: parsed strictly under depth budgets, never executed. Filesystem layout is benign: git checkouts, no other users on the machine. |
| Concurrency | Several agents at once in separate worktrees, each with its own service. Several CLI invocations against one service, which serializes them. Files edited by the author or an agent between two queries, never during an apply. |
| Topology | One machine. The CLI, one background service per configuration over a Unix socket, and a child process to start the service. macOS and Linux. No network. |
| Lifetime | A CLI run lasts milliseconds to seconds; a service lives until ten idle minutes pass. A saved plan is applied in the same session with the same build. The tool owns no durable state: runtime sockets and locks, saved plans, and harness pass-trees are all disposable. |
| Failure cost | The author's time. A wrong edit is recovered from git; a wrong answer misleads one agent step. A service killed by a limit rebuilds on the next query. |
| Compatibility | The author's agents, through the plugin skill and the shared `AGENTS.md` block. The Node harness and Command Center validation depend on exit codes 0/1/2. Its own plan format within one build. No external consumers: nothing is published or tagged. |
| Scale | One very large TypeScript project at a time, on the order of 4,000 to 8,000 program inputs and 460,000 declarations. Two services resident on a 16 GiB machine. Warm queries in the 100 to 200 ms range. Retained memory per service is about 2 GB today, which is the working ceiling. |

## Outside the envelope

Conditions this code will not meet, so no mechanism is added for them:

- Other users on the machine, hostile callers, authentication.
- A second machine, network transport, container or cloud deployment.
- Operating systems other than macOS and Linux: never Windows, FreeBSD, iOS, or Android.
- External consumers of the library API, the plan format, or the batch format. A client and a service of different builds are refused, not bridged.
- A writer that changes a file's contents while leaving its size, modification time, and inode unchanged.
- Files whose ACLs, extended attributes, flags, group, or ownership must survive an edit.

## Accepted inputs

Refused with a plain error, never classified or recovered from:

- Non-regular files (pipes, devices, sockets) where a source, configuration, manifest, or runtime file is expected.
- Hard links, immutable or hidden flags, and files owned by another user, as edit targets.
- Paths and names that are not valid Unicode; UTF-16 sources; invalid UTF-8.
- Source files longer than `u32::MAX` bytes, and files whose control flow has more than `u32::MAX` nodes or containers.
- Configurations with project references.
- Plans whose build identifier or recorded bytes no longer match.

## Recovery

A failed query costs a rerun. A service that a limit discards rebuilds on the next query. Edited sources live in git: a half-applied batch is restored by the tool when the files are still its own, otherwise reported with its backup paths, and `git checkout` is the last resort. No partial-progress state is kept or reported. The one non-regenerable artifact is a saved plan, which is written as a new file only.

## Current scope and deferrals

Shipped: parse and syntax check, outline and show, references and member references, type queries, body replacement, statement insertion, batch editing with plans and apply, service administration, doctor. Deferred, with what each adds:

- A semantic cache per generation: repeated type and member queries on an unchanged snapshot stop repaying checker initialization and reparse.
- Incremental rebinding after an edit: the first query after a change stops costing a full rebind.
