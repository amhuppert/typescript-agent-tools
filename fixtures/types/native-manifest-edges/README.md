Each case keeps `package.json` as raw text so duplicate fields reach the native
decoder. The harness records two separate questions from TypeScript 7.0.2:

- The `selected` import records the loaded declaration's literal type, native
  error-type/unknown identity, and admitted package source files. Cases with a
  `specifier` field use a package subpath; others use the package root.
- A relative import always reaches `first.d.ts`. Its first written import uses
  Import mode and the containing file uses Require mode, forcing fresh contextual
  module spelling. The missing alias is also checked to finish as native unknown.

This separates `loadModuleFromExports` from reverse export spelling. In
particular, root resolution can select an explicit `.` key even when the export
object has mixed invalid key classes; non-root resolution requires IsSubpaths.
It also exposes `GetVersionPaths`/`GetPaths` behavior for empty ranges, first
matching values, allocated non-string slots, and ordered duplicate reduction.

The harness runs with both configuration-directory and workspace cwd. Captures
are `.cc/temp/native-manifest-edges-{config,workspace}.json`; `expected.json` and
`expected-workspace.json` are promoted only after all rows complete without hook
acquisition failures. The workspace baseline matches Rust's loaded Program
tests without changing the process cwd. No production helper is invoked by this
harness.
