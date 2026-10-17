# Adversarial configuration implementation review

Reviewed 2026-10-10 using `posd:ousterhout-software-design`, its reviewing
route, and `AGENTS.md`. The operating envelope remains a local native Rust
project snapshot. This review does not propose a compiler backend, plugin
execution, or a separate configuration service.

The reviewed slice comprises `src/project/program/config.rs`, its target
comparison in `libraries.rs`, snapshot retention through `Program` and
`Project`, `tests/type_configuration.rs`, the private configuration snapshot
tests, and `docs/types-options.md`. Reviewed file SHA-256 values and an
independent catalog extraction are recorded in
`.cc/temp/config-review/catalog.json`.

**Verdict:** the two concrete findings below have been fixed. No additional
blocking defect was found in this configuration slice. This verdict concerns
conversion, inheritance, final project loading, and retained inputs. It does
not establish option-dependent T/Q3/Q4 answers, demanded deferred-feature
refusals, the native checker gate, or complete compiler-diagnostic parity.

## Findings and dispositions

### P1, resolved: `checkJs` silently omitted JavaScript roots

The initial `Options.allow_js` read only `allowJs`, defaulting to false. The
pinned `CompilerOptions.GetAllowJS` instead uses an explicit `allowJs` first
and otherwise enables JavaScript when `checkJs` is true. A project containing
`a.ts` and `b.js` therefore loaded successfully while omitting `b.js` under
`checkJs:true`. The compiler includes both roots. Until JavaScript is
supported, this tool must report the demanded JavaScript input as Unsupported
rather than answer from a smaller program.

The decision now lives in `Reader::allow_js`, with the TypeScript function
named in its comment. Both the loader and retained semantic input receive its
effective result. The test
`check_js_enables_js_inputs_unless_allow_js_is_explicitly_false` crosses
checkJs true/false/null with allowJs unset/true/false/null, plus inherited true,
explicit false, and null resets of both settings, asserting final
`Project::load` outcomes. The implementer recorded RED `vrun-20e6d46f` and
GREEN `vrun-f62c7a7a`; the expanded integration file also passed in the later
registered combined run `vrun-6f84da95`.

### P2, resolved: wrong-kind values became identity refusals

The new primitive catalog reused `type_error`, which called
`File::not_string` for every expected kind. Thus `strict:"\ud800"` and
`checkers:"\ud800"` returned Unsupported for unrepresentable identity.
These values are strings where a boolean or number is required; no identity
lookup is involved. Fresh pinned compiler sessions report TS5024, including
when a later valid occurrence overrides the malformed one.

This is an information-leakage problem: exact-name handling had absorbed the
different decision about an option's expected value kind. The common
`type_error` now returns Config for malformed booleans, numbers, lists, and
objects while preserving the exact-string policy where a string is consumed.
The fix keeps one owner instead of special-casing each catalog entry.

The independently owned `tests/type_configuration_review.rs` confirmed RED
`vrun-792d84a7-561c-48cb-b768-31d8d125b298` on the final error-kind assertion,
then GREEN `vrun-2482e547-d9ae-49a8-adf6-b7dea53361f7`: one file matched,
zero skipped tests. Its variants cover ordinary text, isolated high and low
surrogates, a paired astral character, malformed own/repeated/inherited
values, and malformed plugin entries. The later edit from a boolean
`then` expression to an explicit `if` only removes a Clippy warning.

## Independent compiler and source evidence

The pinned source is TypeScript 7.0.2 at
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`. The source-extracted catalog has
124 distinct canonical names. The Rust catalog contains all 124, with matching
primitive/enum/list/object and command-line-only classifications. Its sole
extra is the explicitly documented internal `noDtsResolution` profile rule.
The inventory table also contains 124 distinct names exactly once.

`.cc/temp/config-review/probe.mjs` ran **54 fresh native API sessions**,
checking configuration parsing, program diagnostics, compiler options, and
root-file selection against direct Rust CLI loads. It pins the package and
binary to 7.0.2 and closes every session/snapshot. Complete results are in
`.cc/temp/config-review/results.json`; this bounded diagnostic probe is not a
substitute for the registered test suite.

The probes cover unknown null/non-null keys, empty keys, command-line-only
nulls, strict/default/null inheritance, duplicate values, malformed overwritten
values, removed false modes, outFile including the empty path, removed modes
overridden by valid values, newLine enumerations, plugins, fractional/negative
numbers, list nulls and falsy entries, moduleSuffixes, paths, null-sensitive
strict prerequisites, GetAllowJS, and the internal noDtsResolution profile.
No unrecorded acceptance difference remained after the two fixes.

Three acceptance differences are deliberate and recorded in the option
inventory: the compiler accepts JavaScript roots while this tool refuses
them; `checkJs:true` with explicit `allowJs:false` reports native TS5052 while
this tool retains the settings and follows its limited combination-diagnostic
policy; explicit true/false `noDtsResolution` is an internal unsupported
profile input here, while the native config parser reports an unknown option.
The combination-diagnostic policy must not be mistaken for a wrong GetAllowJS
result or used to infer that the compiler considers the configuration valid.

## Final ownership and retained-state assessment

The configuration module keeps exact canonical keys, every-occurrence shape
validation, conversion, inheritance, defaulting, and effective overrides
together. Specialized enum/list/path operations remain in the existing
Reader, preserving the preexisting distinctions between duplicate nulls and
null resets during inheritance. The catalog does not execute plugins or
borrow unrelated command-line number constraints.

`SemanticOptions` reaches the final `Project` once; future checker consumers
will not reopen config files or reinterpret raw strict. The eight strict
flags and independent exactOptional/noUnchecked/stable settings match pinned
defaults. Demanded deferred inputs are retained for package deduplication,
JavaScript, decorators, metadata, and configured JSX names.
`GetUseDefineForClassFields` and `GetEmitStandardClassFields` are retained as
distinct operations: explicit true below ES2022 affects the first without
enabling the second. Their shared ES2022 target comparison stays with Target.
No invented false-mode ordering algorithm is attached to stableTypeOrdering,
whose checker consumer has not been found in the pinned implementation.

The six private `semantic_options_*` tests and migrated compiler-probe test
were actually executed and passed in registered checkpoint
`vrun-273725ff-1368-4547-a3d8-16cca5eb8634`; the durable output was inspected.
That **overall checkpoint failed** later on the old navigation CLI
`--references --json` usage expectation, so it is not a full-pass claim.
An attempt to scope the registered runner to `src/project/program/config.rs`
was rejected before executing tests (`vrun-5519a0d8`), because this wrapper
accepts integration-test paths only. No validation success is inferred from
that attempt.

The remaining obligations are implementation checkpoints, not new scope:
positive option-dependent type results, located refusals for demanded deferred
features, refresh/session retention, and the required old-operation
compatibility measurements. Retaining flags alone does not satisfy these
obligations.
