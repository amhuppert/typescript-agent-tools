//! `Project::definition` through `Project::execute`, on the probe of the
//! definitions design (`docs/definition-and-import-hints-design.md`, "The
//! probe"). Every expected answer is the design's: TypeScript 7.0.2's, or
//! the known difference (K1-K8) it documents for that row, never this
//! tool's own output.
mod support;

use support::Fixture;
use typescript_parser::analysis::{
    AnalysisError, InputErrorKind, QueryErrorKind, SemanticErrorKind,
};
use typescript_parser::project::{Project, ProjectRequest, ProjectResponse};
use typescript_parser::references::{
    DefinitionAnswer, DefinitionSelectionFailure, ReferenceAnswer, ReferenceBaseline,
    ReferenceQuery, ReferenceRequest,
};
use typescript_parser::source::SourcePosition;

const CONFIG: &str = r#"{
  "compilerOptions": {
    "strict": true,
    "target": "es2022",
    "module": "nodenext",
    "moduleResolution": "nodenext",
    "jsx": "preserve"
  },
  "include": [
    "src"
  ]
}
"#;

/// The probe's sources, without `src/missing.ts`, whose unresolved import
/// fails every query's load (row 14).
const PROBE: &[(&str, &str)] = &[
    ("tsconfig.json", CONFIG),
    (
        "src/lib.ts",
        "// leading comment
export function formatTitle(title: string): string {
  return title.trim();
}
export function over(a: string): string;
export function over(a: number): number;
export function over(a: any): any {
  return a;
}
export class Box {
  constructor(public size: number) {}
}
export interface Box {
  label?: string;
}
over(1);
new Box(1);
",
    ),
    ("src/def.ts", "export default function makeThing() {}\n"),
    ("src/anon.ts", "export default 42;\n"),
    ("src/anonfn.ts", "export default function () {}\n"),
    ("src/deep.ts", "export const deepValue = 1;\n"),
    ("src/more.ts", "export * from \"./deep\";\n"),
    (
        "src/index.ts",
        "export { formatTitle } from \"./lib\";\nexport * from \"./more\";\n",
    ),
    ("src/eq.ts", "const config = { a: 1 };\nexport = config;\n"),
    ("src/eqobj.ts", "// leading comment\nexport = { a: 1 };\n"),
    (
        "src/barrel.ts",
        "import { formatTitle } from \"./lib\";
export { formatTitle };
export * as star from \"./lib\";
export default formatTitle;
export { formatTitle as renamed } from \"./lib\";
",
    ),
    (
        "src/app.ts",
        "import { formatTitle, deepValue } from \"./index\";
import { formatTitle as fmt, over, Box } from \"./lib\";
import makeThing from \"./def\";
import anon from \"./anon\";
import anonFn from \"./anonfn\";
import * as lib from \"./lib\";
import eq = require(\"./eq\");
import eqObj = require(\"./eqobj\");
import { star, renamed } from \"./barrel\";
import dflt from \"./barrel\";

const h = formatTitle(\" Docs \");
const f = formatTitle;
const g = fmt;
over(\"s\");
new Box(2);
makeThing();
lib.formatTitle(\"x\");
lib.over(3);
new lib.Box(4);
export const n = anon + deepValue + eq.a;
export const values = [anonFn, eqObj, star, renamed, dflt, h, f, g];
",
    ),
    (
        "src/bind.ts",
        "const source = { a: 1 };
const { a } = source;
interface Shape { b: number }
const b = 1;
export const value: Shape = { b };
export const used = a;
",
    ),
    (
        "src/short.ts",
        "import { formatTitle } from \"./lib\";
const source = { a: 1, b: 2 };
const { ...rest } = source;
export const obj = { formatTitle };
export const all = rest;
export type T = typeof import(\"./lib\").formatTitle;
export const key = source[\"a\"];
",
    ),
    (
        "src/kw.ts",
        "class Base {
  constructor(public n: number) {}
}
class Child extends Base {
  constructor() {
    super(1);
  }
  static {
    void 0;
  }
  run(x: number): number {
    outer: for (;;) {
      break outer;
    }
    switch (x) {
      case 1:
        return this.n;
    }
    return 0;
  }
}
export const shorthand = { n: 1, \"q\": 2 };
export function named() {}
export { \"f-g\" } from \"./strings\";
",
    ),
    (
        "src/strings.ts",
        "const fg = 1;\nexport { fg as \"f-g\" };\n",
    ),
    (
        "src/jsx.tsx",
        "function Item(props: { label: string }) {
  return null as any;
}
export const item = <Item label=\"x\" />;
export const link = <svg xlink:href=\"#x\" />;
",
    ),
    (
        "src/nocheck.ts",
        "// @ts-nocheck\nimport { gone } from \"./gone-module\";\nexport const g2 = gone;\n",
    ),
];

/// The probe, loaded.
fn probe() -> (Fixture, Project) {
    let fixture = Fixture::new(PROBE);
    let project = fixture.load();
    (fixture, project)
}

fn definition(project: &Project, query: ReferenceQuery) -> Result<DefinitionAnswer, AnalysisError> {
    project
        .execute(ProjectRequest::Definition(query))
        .map(|response| match response {
            ProjectResponse::Definition(answer) => *answer,
            other => panic!("a definition request answered {other:?}"),
        })
}

/// The position `FILE:LINE:COLUMN`, in the probe's `src`.
fn at(fixture: &Fixture, position: &str) -> ReferenceQuery {
    let (file, rest) = position.split_once(':').unwrap();
    let (line, column) = rest.split_once(':').unwrap();
    ReferenceQuery::At {
        file: fixture.path(&format!("src/{file}")),
        position: SourcePosition {
            line: line.parse().unwrap(),
            column: column.parse().unwrap(),
        },
    }
}

/// Each group's declarations, as `NAME@FILE:LINE:COLUMN` with FILE in the
/// probe's `src`.
fn groups(answer: &DefinitionAnswer) -> Vec<Vec<String>> {
    let DefinitionAnswer::Selected(result) = answer else {
        panic!("expected declarations: {answer:?}");
    };
    result
        .groups
        .iter()
        .map(|group| {
            group
                .declarations
                .iter()
                .map(|declaration| {
                    let location = &declaration.location;
                    format!(
                        "{}@{}:{}:{}",
                        declaration.name,
                        location.file.strip_prefix("src/").unwrap_or(&location.file),
                        location.start.line,
                        location.start.column
                    )
                })
                .collect()
        })
        .collect()
}

/// Every position of `positions` answers one group with `declarations`.
fn one_group(fixture: &Fixture, project: &Project, positions: &[&str], declarations: &[&str]) {
    for position in positions {
        let answer = definition(project, at(fixture, position))
            .unwrap_or_else(|error| panic!("{position}: {error}"));
        assert_eq!(groups(&answer), [declarations], "{position}");
    }
}

const FORMAT_TITLE: &str = "formatTitle@lib.ts:2:17";
const OVERLOADS: [&str; 3] = ["over@lib.ts:5:17", "over@lib.ts:6:17", "over@lib.ts:7:17"];
const BOX: [&str; 2] = ["Box@lib.ts:10:14", "Box@lib.ts:13:18"];

/// Rows 1-4, 7, 8, 10, 11, and 13: the original declaration behind every
/// import and export alias, as TypeScript 7.0.2 answers.
#[test]
fn imports_and_exports_answer_their_original_declarations() {
    let (fixture, project) = probe();
    let rows: &[(&[&str], &[&str])] = &[
        // 1: a use of a name imported through a barrel, called and not.
        (&["app.ts:12:11", "app.ts:13:11"], &[FORMAT_TITLE]),
        // 2: the local name of an import specifier.
        (&["app.ts:1:10"], &[FORMAT_TITLE]),
        // 3: a renaming import: its property name, local name, and a use.
        (
            &["app.ts:2:10", "app.ts:2:25", "app.ts:14:11"],
            &[FORMAT_TITLE],
        ),
        // 4: default imports of a named default function and of a name.
        (&["app.ts:3:8", "app.ts:17:1"], &["makeThing@def.ts:1:25"]),
        (&["app.ts:10:8"], &[FORMAT_TITLE]),
        // 7: a namespace import's member that resolves lexically.
        (&["app.ts:18:5"], &[FORMAT_TITLE]),
        // 8: `import eq = require()` of a module ending `export = config`.
        (&["app.ts:7:8", "app.ts:21:37"], &["config@eq.ts:1:7"]),
        // 10: re-exports, a local export of an import, both names of a
        // renaming re-export, and an import of the renamed name.
        (
            &[
                "index.ts:1:10",
                "barrel.ts:2:10",
                "barrel.ts:5:10",
                "barrel.ts:5:25",
                "app.ts:9:16",
            ],
            &[FORMAT_TITLE],
        ),
        // 11: a name reached through two `export *` declarations.
        (
            &["app.ts:1:23", "app.ts:21:25"],
            &["deepValue@deep.ts:1:14"],
        ),
        // 13: the name in `export = config`.
        (&["eq.ts:2:10"], &["config@eq.ts:1:7"]),
    ];
    for (positions, declarations) in rows {
        one_group(&fixture, &project, positions, declarations);
    }
}

/// Rows 5, 6, 9, and 12 (K2): a declaration without a name keeps the
/// reference model's anchor, where TypeScript starts at its first token.
#[test]
fn declarations_without_a_name_answer_their_anchors() {
    let (fixture, project) = probe();
    let rows: &[(&[&str], &[&str])] = &[
        // 5: `export default 42;` and `export default function () {}`.
        (&["app.ts:4:8", "app.ts:21:18"], &["default@anon.ts:1:8"]),
        (&["app.ts:5:8"], &["default@anonfn.ts:1:8"]),
        // 6: a namespace import, its name and a use.
        (&["app.ts:6:13", "app.ts:18:1"], &["src/lib.ts@lib.ts:1:1"]),
        // 9: `export = { a: 1 };`.
        (&["app.ts:8:8"], &["export=@eqobj.ts:2:8"]),
        // 12: `export * as star` and an import of `star`.
        (
            &["barrel.ts:3:13", "app.ts:9:10"],
            &["src/lib.ts@lib.ts:1:1"],
        ),
    ];
    for (positions, declarations) in rows {
        one_group(&fixture, &project, positions, declarations);
    }
}

/// Rows 15-17 (K1): a declaration's own name answers every declaration of
/// its symbol, and so does every call and `new`, however the callee is
/// reached: no signature is selected and no constructor added.
#[test]
fn declarations_calls_and_new_answer_every_declaration_of_the_symbol() {
    let (fixture, project) = probe();
    let rows: &[(&[&str], &[&str])] = &[
        // 15: a function, an overload and the implementation, and a class
        // merged with an interface.
        (&["lib.ts:2:17"], &[FORMAT_TITLE]),
        (&["lib.ts:5:17", "lib.ts:7:17"], &OVERLOADS),
        (&["lib.ts:10:14"], &BOX),
        // 16a, 16b: calls of the overloaded function through an import
        // alias, locally, and through a namespace import.
        (&["app.ts:15:1", "lib.ts:16:1", "app.ts:19:5"], &OVERLOADS),
        // 17a, 17b: `new Box` through an import alias, locally, and
        // through a namespace import.
        (&["app.ts:16:5", "lib.ts:17:5", "app.ts:20:9"], &BOX),
    ];
    for (positions, declarations) in rows {
        one_group(&fixture, &project, positions, declarations);
    }
}

/// Rows 18-21 (K8): a binding element's name declares the binding, and a
/// shorthand property name is a use of its value, following an import.
#[test]
fn binding_elements_and_shorthand_names_answer_their_bindings() {
    let (fixture, project) = probe();
    let rows: &[(&[&str], &[&str])] = &[
        // 18: `const { a } = source` and a use of `a`.
        (&["bind.ts:2:9", "bind.ts:6:21"], &["a@bind.ts:2:9"]),
        // 19: the shorthand `{ b }`, with a contextual type.
        (&["bind.ts:5:31"], &["b@bind.ts:4:7"]),
        // 20: the shorthand `{ formatTitle }` of an imported name.
        (&["short.ts:4:22"], &[FORMAT_TITLE]),
        // 21: a rest element's name and a use.
        (&["short.ts:3:12", "short.ts:5:20"], &["rest@short.ts:3:12"]),
    ];
    for (positions, declarations) in rows {
        one_group(&fixture, &project, positions, declarations);
    }
}

/// `--name` selects every candidate spelled the name: FILE narrows to the
/// candidates in it, among them an import specifier that declares no name.
#[test]
fn a_name_selects_its_candidates_and_file_narrows_them() {
    let (fixture, project) = probe();
    let name = |name: &str, file: Option<&str>| {
        definition(
            &project,
            ReferenceQuery::Name {
                name: name.into(),
                file: file.map(|file| fixture.path(&format!("src/{file}"))),
            },
        )
        .unwrap_or_else(|error| panic!("{name} {file:?}: {error}"))
    };
    // The file only imports the name; aliases of one symbol are one group.
    for file in ["app.ts", "short.ts", "index.ts", "barrel.ts"] {
        assert_eq!(
            groups(&name("formatTitle", Some(file))),
            [[FORMAT_TITLE]],
            "{file}"
        );
    }
    assert_eq!(groups(&name("formatTitle", None)), [[FORMAT_TITLE]]);
    assert_eq!(
        groups(&name("deepValue", Some("app.ts"))),
        [["deepValue@deep.ts:1:14"]]
    );
    // `export * as star`, whose name the catalog does not key.
    assert_eq!(
        groups(&name("star", Some("barrel.ts"))),
        [["src/lib.ts@lib.ts:1:1"]]
    );
    // Two shadowed declarations matched by one name give two groups, in
    // the order of their first declarations.
    assert_eq!(
        groups(&name("source", None)),
        [["source@bind.ts:1:7"], ["source@short.ts:2:7"]]
    );
    assert_eq!(groups(&name("a", Some("bind.ts"))), [["a@bind.ts:2:9"]]);
}

/// Row 14 (K3): an unresolved module fails the load in a file TypeScript
/// checks, and makes the name it imports unresolved in a skipped file.
#[test]
fn an_unresolved_module_fails_the_load_or_the_name() {
    let (fixture, project) = probe();
    let error = definition(&project, at(&fixture, "nocheck.ts:3:19")).unwrap_err();
    let AnalysisError::Query(error) = error else {
        panic!("expected a query failure: {error}");
    };
    assert_eq!(error.kind, QueryErrorKind::UnresolvedTarget);
    assert_eq!(
        error.to_string(),
        format!(
            "{}:3:19: `gone`: module is not resolved",
            fixture.path("src/nocheck.ts").display()
        )
    );
    let mut files = PROBE.to_vec();
    files.push((
        "src/missing.ts",
        "import { missing } from \"./missing-module\";\nexport const m = missing;\n",
    ));
    let missing = Fixture::new(&files);
    let error = Project::load(&missing.path("tsconfig.json")).unwrap_err();
    let AnalysisError::Input(error) = error else {
        panic!("expected an input failure: {error}");
    };
    assert_eq!(error.kind, InputErrorKind::UnresolvedImport);
    assert_eq!(
        error.to_string(),
        format!(
            "{}:1:25: cannot resolve module \"./missing-module\"",
            missing.path("src/missing.ts").display()
        )
    );
}

/// A result echoes its selection with files relative to the configuration's
/// directory, as a reference result does.
#[test]
fn a_result_names_its_project_and_echoes_its_selection() {
    let (fixture, project) = probe();
    let DefinitionAnswer::Selected(result) =
        definition(&project, at(&fixture, "app.ts:1:10")).unwrap()
    else {
        panic!("expected a result");
    };
    assert_eq!(result.project, fixture.path("tsconfig.json"));
    assert_eq!(
        result.selection,
        ReferenceQuery::At {
            file: "src/app.ts".into(),
            position: SourcePosition {
                line: 1,
                column: 10
            },
        }
    );
    let declaration = &result.groups[0].declarations[0];
    assert_eq!(declaration.location.file, "src/lib.ts");
    assert_eq!(
        (
            declaration.location.end.line,
            declaration.location.end.column
        ),
        (2, 28)
    );
}

/// The declarations a position's answer is not, as `member`'s name or the
/// failure's kind and message.
#[derive(Debug, PartialEq, Eq)]
enum Unselected {
    Member(String),
    NotFound,
}

fn unselected(answer: DefinitionAnswer) -> Unselected {
    match answer {
        DefinitionAnswer::Unselected(selection) => match selection.outcome {
            DefinitionSelectionFailure::Member { name } => Unselected::Member(name),
            DefinitionSelectionFailure::NotFound => Unselected::NotFound,
        },
        DefinitionAnswer::Selected(result) => panic!("expected no declarations: {result:?}"),
    }
}

/// K4: every position the reference selection sends to its member path
/// answers `member`, naming the member that `--references --at` there
/// selects, and a reference search at the same position searches it.
#[test]
fn a_member_position_answers_member_and_its_reference_search_selects_it() {
    let (fixture, project) = probe();
    for (position, name, declaration) in [
        // A property of a value, `eq.a`.
        ("app.ts:21:40", "a", "eq.ts:1:18"),
        // A property access on `this`.
        ("kw.ts:17:21", "n", "kw.ts:2:22"),
        // A member declaration's name.
        ("kw.ts:11:3", "run", "kw.ts:11:3"),
        // A parameter property's modifier.
        ("kw.ts:2:15", "n", "kw.ts:2:22"),
        // A string literal member name and a literal element access key.
        ("kw.ts:22:35", "q", "kw.ts:22:34"),
        ("short.ts:7:28", "a", "short.ts:2:18"),
    ] {
        let answer = definition(&project, at(&fixture, position))
            .unwrap_or_else(|error| panic!("{position}: {error}"));
        assert_eq!(
            unselected(answer),
            Unselected::Member(name.into()),
            "{position}"
        );
        // The search the member outcome names selects the same member,
        // whose declaration TypeScript 7.0.2 answers there.
        let search = project
            .references(ReferenceRequest {
                query: at(&fixture, position),
                filters: Default::default(),
            })
            .unwrap_or_else(|error| panic!("{position}: {error}"));
        let ReferenceAnswer::Selected(result) = search else {
            panic!("{position}: {search:?}");
        };
        assert_eq!(
            result.coverage.baseline,
            ReferenceBaseline::Member,
            "{position}"
        );
        let declarations: Vec<String> = result
            .groups
            .iter()
            .flat_map(|group| &group.declarations)
            .map(|declaration| {
                let location = &declaration.location;
                format!(
                    "{}:{}:{}",
                    location.file.strip_prefix("src/").unwrap(),
                    location.start.line,
                    location.start.column
                )
            })
            .collect();
        assert!(
            declarations.iter().any(|found| found == declaration),
            "{position}: {declarations:?}"
        );
    }
}

/// K4: selecting a member can itself fail, and that failure is the answer:
/// a JSX attribute's deferred typing, and, today, the `Internal` failure
/// at every character of a JSX namespaced attribute, its colon included.
#[test]
fn a_member_whose_selection_fails_answers_that_failure() {
    let (fixture, project) = probe();
    let AnalysisError::Semantic(error) =
        definition(&project, at(&fixture, "jsx.tsx:4:27")).unwrap_err()
    else {
        panic!("expected a semantic failure");
    };
    assert_eq!(error.kind, SemanticErrorKind::UnsupportedSemantic);
    assert_eq!(
        error.to_string(),
        format!(
            "{}:4:27: a JSX attribute relates to its props member only through deferred JSX attribute typing",
            fixture.path("src/jsx.tsx").display()
        )
    );
    for position in ["jsx.tsx:5:26", "jsx.tsx:5:31", "jsx.tsx:5:32"] {
        let AnalysisError::Semantic(error) =
            definition(&project, at(&fixture, position)).unwrap_err()
        else {
            panic!("{position}: expected a semantic failure");
        };
        assert_eq!(error.kind, SemanticErrorKind::Internal, "{position}");
        assert_eq!(
            error.message, "canonical semantic declaration association is not retained",
            "{position}"
        );
    }
}

/// K5 and K6: a position in no identifier and no member site, such as a
/// keyword, whitespace, a comment, or a string literal that names no
/// member, selects nothing: `not_found`, never an empty result.
#[test]
fn a_position_in_no_identifier_answers_not_found() {
    let (fixture, project) = probe();
    for position in [
        // `super`, `this`, `return`, and `case`.
        "kw.ts:6:5",
        "kw.ts:17:16",
        "kw.ts:17:9",
        "kw.ts:16:7",
        // `constructor`, and `static` of a class static block.
        "kw.ts:2:3",
        "kw.ts:8:3",
        // `function` and `export` before a declaration's name.
        "kw.ts:23:8",
        "kw.ts:23:1",
        // The `import` of `import eq = require()` and of a declaration.
        "app.ts:7:1",
        "app.ts:1:1",
        // A module specifier, a type's `import("./lib")` argument, and a
        // string specifier name.
        "app.ts:1:41",
        "short.ts:6:32",
        "kw.ts:24:11",
        // Whitespace, an empty line, punctuation, and a comment.
        "app.ts:1:9",
        "app.ts:11:1",
        "app.ts:12:23",
        "lib.ts:1:4",
    ] {
        let answer = definition(&project, at(&fixture, position))
            .unwrap_or_else(|error| panic!("{position}: {error}"));
        assert_eq!(unselected(answer), Unselected::NotFound, "{position}");
    }
    let name = |name: &str, file: Option<&str>| {
        definition(
            &project,
            ReferenceQuery::Name {
                name: name.into(),
                file: file.map(|file| fixture.path(&format!("src/{file}"))),
            },
        )
        .unwrap_or_else(|error| panic!("{name}: {error}"))
    };
    // No candidate: a different case, a member's name, and a name the
    // file only uses.
    assert_eq!(unselected(name("FormatTitle", None)), Unselected::NotFound);
    assert_eq!(unselected(name("trim", None)), Unselected::NotFound);
    assert_eq!(unselected(name("h", Some("lib.ts"))), Unselected::NotFound);
    assert_eq!(
        unselected(name("Base", Some("app.ts"))),
        Unselected::NotFound
    );
}

/// A refused or unresolved token fails the request with the reference
/// selection's error at its location, by position and by name.
#[test]
fn a_refused_or_unresolved_target_fails_with_its_kind() {
    let (fixture, project) = probe();
    let failure = |query: ReferenceQuery| match definition(&project, query) {
        Err(AnalysisError::Query(error)) => (error.kind, error.to_string()),
        other => panic!("expected a query failure: {other:?}"),
    };
    let src = |file: &str| fixture.path(&format!("src/{file}")).display().to_string();
    // K5: a label.
    assert_eq!(
        failure(at(&fixture, "kw.ts:13:13")),
        (
            QueryErrorKind::UnsupportedTarget,
            format!(
                "{}:13:13: `outer` is not a supported reference target (label)",
                src("kw.ts")
            )
        )
    );
    // An unresolved candidate of a name fails at the candidate.
    assert_eq!(
        failure(ReferenceQuery::Name {
            name: "gone".into(),
            file: None,
        }),
        (
            QueryErrorKind::UnresolvedTarget,
            format!("{}:2:10: `gone`: module is not resolved", src("nocheck.ts"))
        )
    );
}

/// K4: a qualified member in a type position is refused as `--references
/// --at` refuses it, and a member with no symbol fails while it is
/// selected, naming no search.
#[test]
fn type_position_members_and_members_without_a_symbol_fail() {
    let fixture = Fixture::new(&[
        ("tsconfig.json", CONFIG),
        (
            "src/a.ts",
            "declare const x: any;\nexport const y = x.foo;\nenum E { A }\nexport let z: E.A;\n",
        ),
    ]);
    let project = fixture.load();
    let failure = |position: &str| match definition(&project, at(&fixture, position)) {
        Err(AnalysisError::Query(error)) => (error.kind, error.to_string()),
        other => panic!("{position}: expected a query failure: {other:?}"),
    };
    let file = fixture.path("src/a.ts").display().to_string();
    assert_eq!(
        failure("a.ts:2:20"),
        (
            QueryErrorKind::UnresolvedTarget,
            format!("{file}:2:20: `foo` has no reference symbol at this site")
        )
    );
    assert_eq!(
        failure("a.ts:4:17"),
        (
            QueryErrorKind::UnsupportedTarget,
            format!("{file}:4:17: `A` is not a supported reference target (enum member)")
        )
    );
}

/// The request's own errors: a file that is not searched, a position
/// outside its file, and a dotted name, each before anything is selected.
#[test]
fn files_positions_and_dotted_names_are_refused() {
    let (fixture, project) = probe();
    let kind = |query: ReferenceQuery| match definition(&project, query) {
        Err(AnalysisError::Query(error)) => error.kind,
        other => panic!("expected a query failure: {other:?}"),
    };
    let outside = fixture.path("other.ts");
    assert_eq!(
        kind(ReferenceQuery::At {
            file: outside.clone(),
            position: SourcePosition { line: 1, column: 1 },
        }),
        QueryErrorKind::OutsideProject
    );
    assert_eq!(
        kind(ReferenceQuery::Name {
            name: "formatTitle".into(),
            file: Some(outside),
        }),
        QueryErrorKind::OutsideProject
    );
    assert_eq!(
        kind(at(&fixture, "app.ts:99:1")),
        QueryErrorKind::InvalidPosition
    );
    for name in ["Box.size", "lib.formatTitle", "."] {
        assert_eq!(
            kind(ReferenceQuery::Name {
                name: name.into(),
                file: None,
            }),
            QueryErrorKind::InvalidRequest,
            "{name}"
        );
    }
}
