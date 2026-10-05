# Find references

`tsparse --references` groups the declarations and occurrences of a symbol in
one configured TypeScript project. The Rust library exposes the same operation
as `typescript_parser::references::ReferenceProject`. The design is recorded in
[find-references-design.md](find-references-design.md); this document describes
the implemented behavior and where it deliberately differs from TypeScript.

## Command

```sh
tsparse --references --project ./tsconfig.json --at 12:8 src/app.ts
tsparse --references --project ./tsconfig.json --name run
tsparse --references --project ./tsconfig.json --name run src/app.ts --pretty
```

`--project` names a configuration file; no ancestor directory is searched.
Exactly one of `--at LINE:COLUMN` (one-based line, one-based UTF-16 column, with
a required FILE) or `--name NAME` (with an optional FILE that narrows the
candidates) selects the symbols. Argument paths are relative to the working
directory. `--pretty` and `-o OUTPUT` keep their parse-mode meanings; stdin,
`--check`, and `--tsx` are rejected.

| Exit | Meaning |
| --- | --- |
| 0 | A result, including `groups: []` for a token-free position or an unmatched name. |
| 1 | A syntax error, unsupported required input, invalid binding, unresolved import, or an unsupported or unresolved selected target. |
| 2 | Invalid flags or position, a missing or unreadable configuration, or a query file that is not one of the project's searched files (including a missing file). |

Diagnostics on stderr have the form `path:line:column: message` when a location
is known.

## Library

```rust
use std::path::Path;
use typescript_parser::references::{ReferenceProject, ReferenceQuery, SourcePosition};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let project = ReferenceProject::load(Path::new("tsconfig.json"))?;
    let result = project.find_references(ReferenceQuery::At {
        file: "src/app.ts".into(),
        position: SourcePosition { line: 12, column: 8 },
    })?;
    for group in &result.groups {
        println!("{} occurrences", group.occurrences.len());
    }
    Ok(())
}
```

`load` reads each program input once and returns either a complete immutable
index or a `ProjectError` (`Io`, `Config`, `Syntax`, `Unsupported`, `Binding`,
or `UnresolvedImport`) with a path and position where available. Queries read
no files: a query path is made absolute against the working directory and
matched lexically against each searched file's canonical path and its path
under the configuration directory, so results reflect the captured contents
even if files change afterwards. Queries return `QueryError`
(`OutsideProject`, `InvalidPosition`, `UnsupportedTarget`, `UnresolvedTarget`,
or `Io` when the working directory is unavailable). Results own their data and
can outlive the project.

## Result

```json
{
  "project": "/abs/path/tsconfig.json",
  "groups": [{
    "declarations": [{ "name": "run", "location": { "file": "src/api.ts", "start": { "line": 1, "column": 17 }, "end": { "line": 1, "column": 20 } } }],
    "occurrences": [{ "name": "run", "location": { "...": "..." }, "roles": ["declaration", "export"] }]
  }]
}
```

Each group is one underlying symbol. `declarations` lists all of its
declarations, including merged declarations and declarations in installed
packages. `occurrences` lists the symbol's tokens in the searched files: its
declarations (`declaration`), references (`use`), and names in import and
export clauses (`import`, `export`). A token can carry several roles, such as
`export function run` (declaration and export). A token that denotes more than
one symbol, such as `export { Shape }` naming both an interface and a constant,
appears in each group.

File paths use `/` and are relative to the configuration's directory, with
`../` where needed. Ranges are exact identifier tokens without trivia; `end` is
exclusive. Occurrences are ordered by file and position, groups by their
earliest declaration.

Declarations that have no identifier use an anchor: an anonymous default export
(`export default function () {}`, `export default 42`) is named `default` at the
`default` keyword, `export = expression` is named `export=` at its `=`, a module file used as a namespace (`import * as ns`, `import x =
require()`, `export * as ns`) is named by its path at line 1, column 1, and an
ambient module (`declare module "x"`) is named `x` at its string literal.

## Selection

A position must fall inside an identifier token, start inclusive and end
exclusive. Whitespace, punctuation, and keywords give an empty result. Member,
property, enum member, label, JSX attribute, intrinsic element, tuple label,
import attribute, `import.meta`, `new.target`, `arguments`, `undefined`, and
`globalThis` tokens are `UnsupportedTarget`. A name with no declaration among the
program inputs is `UnresolvedTarget`.

A name query is exact and case-sensitive on decoded identifier text, so
`abc` and `abc` are the same name. Candidates are declaration names, local
import names, and exported alias names in the searched files; FILE restricts
candidates, not results. Every matching candidate's symbols are grouped, so two
shadowed declarations give two groups and two aliases of one symbol give one.
An unresolved candidate makes the query fail with `UnresolvedTarget` rather than
silently omitting it.

## Binding model

Each scope maps a name to at most one symbol per meaning (value, type, and
namespace) plus an optional import or export alias. A reference asks for the
meaning its syntax requires and searches outward, so a type alias and a constant
with the same spelling remain separate groups and a type does not shadow a
value. Declarations that TypeScript merges become one symbol: function
overloads, interfaces, class and interface, namespace with namespace, class,
function, or enum, enums, and redeclared `var` and parameters. Conflicting
declarations, such as two `let x`, fail with a `Binding` error. Conflicts only
among installed-package declarations are merged, as TypeScript's binder does
when library checking is skipped.

Scopes follow TypeScript: function-scoped `var` and block-scoped `let`, `const`,
`using`, classes, and strict-mode functions; separate parameter and body scopes,
so a parameter default cannot see body declarations; class and interface type
parameters shared by merged declarations; mapped type parameters; and `infer`
declarations visible only in their conditional type's true branch. Namespace
blocks see each other's exports but not each other's locals. Script files share
the global scope with `declare global` blocks. Decorators resolve where their
class is declared.

Aliases are directed edges. An import, re-export, `export =`, `import =`
alias, `export as namespace` global, or `export * as` name resolves to the
underlying symbol, following star re-exports (local exports shadow them, and
different targets for one name are ambiguous) and `export =` targets. A
type-only alias does not supply a value outside type positions, so
`import type { C }` followed by `new C()` leaves `C` unresolved. `ns.name` and
`N.Type` resolve through module and namespace exports; other property accesses
are not reference targets. Ordinary value flow, such as `const copy = run`,
creates no alias.

Aliases and qualified references resolve on demand with an explicit task stack;
a circular alias is unresolved rather than an error.

## Project configuration

The configuration is JSONC (comments and trailing commas). `extends` accepts a
string or an ordered array of relative, rooted, or package configurations
(through `node_modules`, a manifest `tsconfig` field, `exports`, or a package's
`tsconfig.json`), with `${configDir}` and inheritance cycles reported as
configuration errors. Path settings stay relative to the configuration that
declared them; `files`, `include`, and `exclude` replace inherited values.

Roots follow TypeScript's selection: `files`, `include` and `exclude` globs,
`foo.ts` hiding `foo.d.ts`, and default excludes of `outDir` and
`declarationDir`. Program inputs add every resolved import, re-export,
`import = require`, import type, dynamic `import("literal")`, module
augmentation, `/// <reference path>`, `/// <reference types>`, and `types`
entry, plus the automatic JSX runtime of each `.tsx` file under `jsx`
`react-jsx` or `react-jsxdev`, from `jsxImportSource` or a block-comment
`@jsxImportSource` pragma (`@jsxRuntime classic` turns it off). Searched files are the roots and their local dependencies; files reached
through packages or type packages are program inputs whose declarations
resolve names but whose own occurrences are not reported.

Module resolution implements TypeScript 7's `bundler` (the default), `node16`,
and `nodenext` modes: importer format, per-request resolution mode, extension
substitution, directory and extensionless rules, `paths`, `rootDirs`,
`moduleSuffixes`, `node_modules` and `@types` lookup, package `exports`,
`imports`, `typesVersions`, `types`/`typings`/`main`, self-name imports,
`customConditions`, and `resolution-mode` on type-only imports and import
types. A cyclic package `imports` chain is unresolved, and one longer than
16,384 redirects is unsupported. A file's identity is its real path unless `preserveSymlinks` is set, so
a file reached through a symlinked directory is one input where TypeScript can
create two. Exact ambient modules (`declare module "x"`) win over files for
non-relative names, and pattern ambient modules (`"*.css"`) satisfy any
otherwise unresolved name.

`types` defaults to `[]`; list packages, or use `"types": ["*"]` for every type
root. An explicit `typeRoots` turns off the `node_modules` fallback for type
references. `moduleDetection` defaults to `force` for Node.js `module` settings
and `auto` otherwise. Options that do not affect the program graph or binding
are ignored.

Rejected configurations: `baseUrl` (removed in TypeScript 7), `moduleResolution`
`node`, `node10`, or `classic`, `module` `amd`, `umd`, or `system`, a non-empty
`lib`, `/// <reference lib>` without `noLib`, `noResolve`, `noDtsResolution`,
a non-empty `references`, and JavaScript or JSON roots.

## Compatibility limits

- One configuration. Project references are not followed; a configuration with
  non-empty `references` is rejected.
- Inputs are `.ts`, `.tsx`, `.mts`, `.cts`, and their declaration files.
  JavaScript, JSDoc binding, JSON modules, and asset modules are unsupported
  when resolution needs them.
- Standard libraries are not loaded. An explicit non-empty `lib` setting or a
  `/// <reference lib>` directive is rejected, and globals declared only by a
  standard library, such as `Array` or `console`, are unresolved.
- `with` statements and other syntax without a binding rule are rejected with
  an `Unsupported` error naming the construct.
- Properties, methods, class members, enum members, and inferred receiver types
  are not reference targets.

## Differences from TypeScript 7.0.2

The semantic oracle is TypeScript 7.0.2's language service (see
[validation](validation.md#reference-semantics)). Its results are normalized
to this model before comparison. Normalization keeps every program file's
locations, so it cannot hide a missing or extra occurrence; it names files by
canonical path, as this tool does, unless `preserveSymlinks` is set:

- TypeScript gives every same-named declaration in one table a single symbol.
  This tool keeps separate symbols per meaning unless the declarations merge,
  so `interface X` and `declare var X` are separate groups.
- TypeScript reports several groups for aliases of one symbol; this tool
  reports one group for the underlying symbol. TypeScript also creates a
  temporary symbol for each `import * as ns`; groups are identified by their
  declarations instead.
- A value use of a type-only alias is unresolved here; TypeScript reports an
  error but still resolves it. When star re-exports reach one declaration by
  both a type-only and an ordinary route, the ordinary route makes it a value
  here; TypeScript keeps the first route and may report that error.
- `undefined`, `arguments`, and `globalThis` are built-in non-reference
  targets. A missing export of a namespace, module, or `globalThis` qualifier is
  unresolved; any other unresolved `a.b` name is a property. Receiver types are
  not inferred, so `value.member` is a property even where TypeScript resolves
  it through a variable's `typeof namespace` type.
- A named import from a shorthand ambient module (`declare module "x";`)
  denotes the module, as in TypeScript; a member reached through one is
  unresolved. A default import of a declaration file or ambient module with
  neither a default export nor an `__esModule` export denotes the module
  object, as TypeScript's synthetic defaults do; an ordinary module without
  one has no default.
- TypeScript stores an import and a same-named local declaration of another
  meaning in one table symbol; uses select the meaning they need, and the
  import clause name denotes only the imported declaration.
- Non-reference syntax is unsupported even where TypeScript resolves it: a
  binding-pattern key such as `default` in `const { default: x } = await
  import("m")`, `const` in `as const`, `global` in `declare global`, a `this`
  parameter, and a tuple member label.
- A constructor parameter property's name belongs to the parameter's group;
  TypeScript resolves the name itself to the property.
- Invalid declaration arrangements that TypeScript binds but its checker
  rejects are `Binding` errors here: duplicate implementations (TS2393), a
  function merged with a non-ambient class (TS2813/TS2814), a repeated type
  parameter (TS2300), a hoisted `var` past a block's `let` (TS2481), a name
  declared both exported and local in one container (TS2395), a namespace
  before or in another file from the class or function it merges with
  (TS2434/TS2433), and merged classes or interfaces whose type parameter
  names disagree by position or omit a parameter that no declaration gives a
  default (TS2428). Constraints and default types are not compared.
- An unresolved side-effect import fails the project, as TypeScript reports
  TS2882, unless `noUncheckedSideEffectImports` is false.
- An unresolved module or reference directive leaves the names that depend
  on it unresolved instead of failing the project where TypeScript reports no
  error: in a declaration file under `skipLibCheck`, or after a
  `@ts-expect-error` or `@ts-ignore` comment that ends on an earlier line with
  only blank or line-comment lines between. A block comment's directive must
  be on its last line. A missing automatic JSX runtime is reported once per
  file, at its first JSX tag in source order; TypeScript uses the first tag it
  checks, which can be later in source when a function expression's body is
  checked late.

TypeScript 7.0.2 defaults `types` to `[]`: `@types` packages are included only
when listed in `types`, or all of them with `"types": ["*"]`. The design assumed
automatic discovery when `types` is absent; the implementation follows the
compiler.
