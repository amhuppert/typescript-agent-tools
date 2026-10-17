# Pinned imported-JSON controls

`manifest.json` freezes 23 source inputs for the final registered nine-test run
`vrun-493c9f31-e064-4b24-9b43-641029310824` and hashes the seven archived controls.
The eight tool/hook inputs also have byte-for-byte copies under
`../captured-tooling/`. All source hashes were checked unchanged after the run.

The two raw probe scripts are inspection copies of the scripts run from
`.cc/temp/` with the repository as working directory. Their relative tool
imports and output paths describe that original location; the archived copies
are not standalone executable replay tools. Each probe loaded the version-checked
TypeScript 7.0.2 native API. The main probe records 36 fresh selected demands;
the option probe records 72 option demands, 10 direct JSON-key demands, four
TypeScript-use flags controls and two later-diagnostic history controls. Eighteen
option rows have native Program TS5052 and are not admitted oracle profiles.

The five `json-type-go-controls.json` answers come from the final registered run:
three type demands and the duplicate key/use member pair. Each answer includes
the exact frozen replay `inputHashes`; the test checks the six local input bytes
against their filesystem SHA-256. Native checker-source, library and replay
inputs remain identified by the answer's existing provenance. This is a bounded
same-checker control, not a full JSON typing or canonical-group proof.

Raw scripts ran with these pinned compiler paths:

```sh
TYPE_SCRIPT_SEMANTIC_API=/Users/alex/github/typescript-agent-tools/.cache/compiler-oracles/typescript-7.0.2/package
TYPE_SCRIPT_NATIVE=/Users/alex/github/typescript-agent-tools/.cache/compiler-oracles/typescript-darwin-x64-7.0.2/package/lib/tsc
```

The committed fixture is the original source context for the probes. Do not
infer future tool equivalence, performance, recovery causality or arbitrary
input support from these historical artifacts.
