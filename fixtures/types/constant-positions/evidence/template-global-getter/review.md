# Tagged argument continuation: independent source review

Read-only review using the Ousterhout software design skill. The operating envelope is the existing request-owned Evaluator and captured Source. Verdict: **SOURCE PASS** for the bounded tagged argument producer and forwarding hunk. No new substantiated finding.

Pinned checker.go29799 getEffectiveCallArguments acquires TemplateStringsArray, creates a fresh synthetic expression with the actual template parent, and returns that argument followed by the actual template substitution expressions. The existing getter resolver uses arity zero and reportErrors true. Missing globals yield the established emptyObjectType, so native signature checking continues; no success substitute is justified by the healthy selected literal.

The new effective_tagged_template_arguments_completion reads global_object_type_completion in a local NativeSequence. Recovery retains the actual acquired TypeId and its evidence while construction continues. An outer interruption still propagates before synthetic-node construction or substitution extraction. Synthetic context identity is fresh, parent/location stay the actual template, and spread is false. Source's existing getEffectiveCallArguments projection supplies no substitutions for a no-substitution literal and exact captured span expressions for TemplateExpression. The caller returns the full completion directly rather than admitting recovery. No mode, signature-selection, global-owner or spread policy was duplicated or moved.

The existing NativeSequence policy preserves the global cause before later annotation causes and does not append prior evidence twice after a read interruption. This change places completion inside the operation callers already use and hides the native recovery/argument construction behind the same producer boundary. There is no second collector, callback protocol, fallback or global cache.

Evidence overlay review: captured type-oracle.mjs changes getGlobalType to a named return and adds only a deferred read of the already-produced pointer, existing empty-object/empty-generic sentinels, flags, existing ID and pure isErrorType. It does not acquire a type or allocate an ID. The existing bounded AgentOracleObservation receives that row. This supports the recorded empty-object target distinction without adding a semantic demand. Existing optional instrumentation is outside the newly added overlay and was not inferred to have run merely because it is present in the captured toolkit.

Evidence: native vrun-7a19fed5-3833-497c-bcfa-75fff9c8f770 records the healthy and missing getter returns and later error annotation acquisition. Public RED vrun-8fd55be7-f4a1-4d84-a8b6-b3d112e9c050 compiled and failed exactly the strengthened later-cause assertion. Public GREEN vrun-facbabb3-6744-4ad7-9cba-88d4c20ada88 reports five passing constant-position cases, repeated compact/expanded requests, an unrelated healthy literal and generated literal ordering. The precise public assertions were read: the source-free unresolved global precedes an unresolved annotation at its actual UTF-16 span; the selected request remains InvalidSemantic. The run has independent JSON initializer failures, so this verdict does not claim connected initialization or the whole suite is green. No validation was rerun without a new change.

Reviewed source/artifact hashes:

```json
[
  {
    "file": "src/project/semantic/evaluation/calls/tagged.rs",
    "sha256": "0e9c0176af8c9c359248c8f67f88448ba17ef26ca56141e8a024ee0c11c520f7"
  },
  {
    "file": "src/project/semantic/evaluation/calls.rs",
    "sha256": "2ae561e343ba8eb84eaf2f8a2aa5a035563efd313abb15d2032b5ebd2f4fa3c5"
  },
  {
    "file": "src/project/semantic/model/completion.rs",
    "sha256": "96ea6a774d1c2a17c562b79018aa1eff940560d787bbfe1efc5cf18c2d21d980"
  },
  {
    "file": "src/project/semantic/evaluation/globals.rs",
    "sha256": "0587b37d801bb9676e78ebe2beef74ff4f666b2560e5236e5df8f685b41cbeab"
  },
  {
    "file": "src/project/semantic/source/calls.rs",
    "sha256": "43114775df27aeabe15bba0c34095e58c3a4b72c39a8346ef470f7ce17bccef5"
  },
  {
    "file": "tests/type_constant_positions.rs",
    "sha256": "2f3edafe424989d12bc19b52a4135ec5648eed38a33fef30136f1de18576eed6"
  },
  {
    "file": "fixtures/types/constant-positions/evidence/template-global-getter/captured-tooling/tools/type-oracle.mjs",
    "sha256": "04a94e16ab6ef326456a6b2efbee881dcffaec6f94a2096428c8b10438e48aee"
  },
  {
    "file": "fixtures/types/constant-positions/evidence/template-global-getter/observations.json",
    "sha256": "dd3bae3ec0fd86517add6d63acc01e19a11b3dadd9ee637c26e2c0d951b856b3"
  }
]
```
