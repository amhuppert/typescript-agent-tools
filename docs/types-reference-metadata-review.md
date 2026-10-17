# Lexical metadata and member advisory review

The bounded adversarial review found one correctness issue: exact empty member keys were silently excluded by selector matching. The implementation owner fixed it after a failing final-query reproduction. Six independent integration tests now pass. No further blocking issue was found in the reviewed lexical metadata, advisory, and direct/service paths. This review does not establish semantic member references, Q4, native checking, or their performance and resource contracts.

The review used the Ousterhout reviewing route and `AGENTS.md`. Its operating envelope is the existing immutable Project, lexical binding index, captured Q2 selection indexes, and serialized refreshable service. The review owned `tests/reference_metadata_review.rs` and this document; production changes remained with the implementation owner.

## Finding and disposition

**Medium — an exact member key was interpreted as a selector.** The initial `Index::member_candidates` called `select_name`, which rejects empty selector segments. A catalog member with the exact key `""` remains a valid name even though the selector language cannot spell it. Consequently, a bare `ReferenceQuery::Name { name: "", .. }` returned zero suggestions for an object property and class field with that name. This lost advisory declarations and confused two different questions in one operation.

The behavior-level reproduction is `exact_empty_member_names_have_advisories_with_unspellable_selectors`. Registered single-file RED run `vrun-a27ac9db-26e6-4771-9e37-25253777c01d` compiled and executed it, failing on final `Project::references` JSON: expected advisory count 2, received 0. This was not a setup/import failure.

The implementation owner changed `member_candidates` to match each indexed site's exact decoded key and member kind, retaining Q2's candidate/selector/anchor projection. `select_name` still owns selector-language matching. No second decoder, catalog, parser, or binding rule was added. The fix preserves empty-key suggestions with null selectors and replayable Q2 anchors. The final registered GREEN is `vrun-ee163f43-8476-49f3-8cd7-6f7a7a2faeac`: one explicit integration file, six tests, zero skips.

The small interface change fits the envelope: the catalog owner exposes a distinct exact-key question while preserving the existing selector operation. Moving the advisory into a separate parser or interpreting labels as names would have created another decision owner and unnecessary state.

## Independent behavior evidence

`tests/reference_metadata_review.rs` exercises the final public query results and CLI/service outputs:

- Empty literal keys produce two null-selector suggestions. Each anchor replays Q2 and yields the exact empty key.
- Hexadecimal `0x10`, exponent `1.6e1`, and quoted `"16"` share the catalog key `"16"` and one display suggestion. Querying the raw spelling `"0x10"` does not match that decoded key. A quoted escaped `run` matches `run`; `Run` stays distinct from `run` and `RUN`.
- An instance declaration and later static declaration sharing `Host.run` produce one display suggestion retaining the earliest anchor. This is display deduplication, not proof that the declarations denote one semantic symbol. Existing implementation tests separately exercise overload, merged-interface, and getter/setter deduplication.
- Matching children beneath two different unspellable keyed ancestors retain separate null-selector suggestions and replay anchors. No label-derived selector is invented.
- Private and astral keys are preserved. A literal U+FFFD key yields its own suggestion; a lone-surrogate key and computed name have no Q2 lookup key and do not gain an identity from their display labels. This is the existing Q2 key policy, not semantic support for those forms.
- A parameter property's advisory Q2 anchor selects its property catalog record. Feeding that same name anchor to lexical references preserves the parameter group. The advisory does not promise Q3 member-role replay; the future property-producing modifier anchor remains a separate semantic role.
- Anchor positions replay across CR, LF, CRLF, U+2028, and U+2029, each combined with space, tab, and NBSP, with and without a leading BOM. A shebang and astral comment distinguish trivia handling and UTF-16 columns: 30 source variants.
- A selected member token retains the lexical `UnsupportedTarget` failure. Comment/whitespace positions remain empty lexical successes with no advisory. An outside-project FILE remains `OutsideProject`. Reporting filters do not convert selection failures into successful metadata.
- Direct and service requests produce identical status codes, stdout bytes, and stderr bytes, including bare empty/numeric names. Complete results retain lexical coverage and empty semantic gaps.
- FILE narrows seeds and advisory declarations, while selected reference results still span two other files. Calls/test filters leave declaration origins and advisory members visible.
- A same-size source edit with restored mtime changes the service advisory and increments its generation. Invalid configuration and source syntax refreshes produce error documents without stale groups/advisories; direct and service bytes remain equal. Restoring valid input publishes a fresh generation and current advice. Success exits 0 and the exercised configuration/syntax failures exit 1.

The empty-key correction was first made green in `vrun-220fbd49-f123-41aa-b358-dbdb31ce4dbd`. Additional replay/failure assertions passed in `vrun-ac06a43e-2f8c-49b4-a317-e6219fa74c8b`. The final run above includes the complete owned file. These scoped runs do not replace the full compatibility checkpoint maintained by the implementation owner.

## Compiler evidence and question boundaries

The disposable native probe `.cc/temp/reference-metadata-key-probe.mjs`, using the existing pinned semantic API bootstrap, records ten TypeScript 7.0.2 declaration-key comparisons in `.cc/temp/reference-metadata-key-probe/answers.json`. It confirms two empty public keys, alternate numeric keys, decoded quoted `run`, exact `Run`, an astral key, and literal U+FFFD. Nine comparisons inspect public symbol keys; the private comparison inspects the native `PrivateIdentifier` syntax key.

The latter distinction matters: the native private syntax key is `#run`, but its canonical symbol name in this fixture is `__#1@#run`. Q2's syntax key is appropriate for display suggestions. Treating it as a project-wide private semantic identity would collapse different classes and is not justified by this review. The probe uses one native checker and direct declaration identity questions; it is not the complete fresh-request checking baseline or a reference-oracle journey.

The advisory's null-selector policy deliberately preserves a Q2 declaration anchor rather than pretending every key can be written in the selector language. It also preserves the legacy bare-name/dotted-name boundary. The new exact-key operation does not introduce dotted semantic selection into the old lexical request.

## Design assessment and remaining limits

The metadata is built at the existing lexical result owner, so both library boundaries and the Project request dispatcher receive the same coverage contract. Advisory lookup uses captured Q2 indexes and existing candidate projection; it does not reparse disk or invoke the checker. Loading, selection, grouping/filtering, catalog keys/selectors, and refresh remain separate owned questions. The exercised failures remain failures instead of being converted into a partial/complete result or a stale snapshot.

The public coverage/gap types describe the design's eventual result contract. Their presence does not establish production semantic gap accounting: these lexical operations emit `baseline: lexical`, `status: complete`, and no semantic gaps. The resource/internal failures deliberately excluded from the gap reason enum must continue to abort the eventual semantic query. No backend trait, second reference registry, or checker-state adapter is justified by the present behavior.

Bare-name advice now scans the captured selection sites. The review proves its output and ownership, not whole-corpus latency or memory. A new lookup index would add retained state and should be justified by representative measurement rather than introduced as a speculative cleanup. Existing operation load/refresh and query performance still require their separate checkpoints.

No semantic relationship, private symbol uniqueness, full token inventory, computed-key evaluation, dotted semantic ambiguity, modifier anchor, factory/contextual/flow result, resource deadline, or queued-query latency claim is made here. Those remain behind the design's native checker and representation gates.
