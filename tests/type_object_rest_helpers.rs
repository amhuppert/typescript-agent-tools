//! Helper lookup is a real checker dependency and does not read helper types.
mod support;
use support::{Fixture, position};
use typescript_parser::analysis::InputErrorKind;
use typescript_parser::project::Project;
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};
const SOURCE: &str = include_str!("../fixtures/types/native-object-rest/helpers/cases.ts");
const SCRIPT: &str = include_str!("../fixtures/types/native-object-rest/helpers/script.ts");
#[test]
fn actual_helper_requests_preserve_options_module_kinds_and_alias_failures() {
    let fixture = Fixture::new(&[
        (
            "native-variable-patterns/globals.d.ts",
            include_str!("../fixtures/types/native-variable-patterns/globals.d.ts"),
        ),
        ("native-object-rest/helpers/cases.ts", SOURCE),
        ("native-object-rest/helpers/script.ts", SCRIPT),
        (
            "native-object-rest/helpers/exact.d.ts",
            include_str!("../fixtures/types/native-object-rest/helpers/exact.d.ts"),
        ),
        (
            "native-object-rest/helpers/wildcard.d.ts",
            include_str!("../fixtures/types/native-object-rest/helpers/wildcard.d.ts"),
        ),
        (
            "native-object-rest/helpers/type-only.d.ts",
            include_str!("../fixtures/types/native-object-rest/helpers/type-only.d.ts"),
        ),
        (
            "native-object-rest/helpers/broken.d.ts",
            include_str!("../fixtures/types/native-object-rest/helpers/broken.d.ts"),
        ),
        (
            "native-object-rest/helpers/tslib-script.d.ts",
            include_str!("../fixtures/types/native-object-rest/helpers/tslib-script.d.ts"),
        ),
        (
            "native-object-rest/helpers/missing.json",
            include_str!("../fixtures/types/native-object-rest/helpers/missing.json"),
        ),
        (
            "native-object-rest/helpers/exact.json",
            include_str!("../fixtures/types/native-object-rest/helpers/exact.json"),
        ),
        (
            "native-object-rest/helpers/wildcard.json",
            include_str!("../fixtures/types/native-object-rest/helpers/wildcard.json"),
        ),
        (
            "native-object-rest/helpers/type-only.json",
            include_str!("../fixtures/types/native-object-rest/helpers/type-only.json"),
        ),
        (
            "native-object-rest/helpers/broken.json",
            include_str!("../fixtures/types/native-object-rest/helpers/broken.json"),
        ),
        (
            "native-object-rest/helpers/script.json",
            include_str!("../fixtures/types/native-object-rest/helpers/script.json"),
        ),
        (
            "native-object-rest/helpers/disabled.json",
            include_str!("../fixtures/types/native-object-rest/helpers/disabled.json"),
        ),
        (
            "native-object-rest/helpers/newer.json",
            include_str!("../fixtures/types/native-object-rest/helpers/newer.json"),
        ),
        (
            "native-object-rest/helpers/script-source.json",
            include_str!("../fixtures/types/native-object-rest/helpers/script-source.json"),
        ),
        (
            "native-object-rest/helpers/isolated.json",
            include_str!("../fixtures/types/native-object-rest/helpers/isolated.json"),
        ),
    ]);
    for config in [
        "missing",
        "exact",
        "wildcard",
        "type-only",
        "broken",
        "script",
        "disabled",
        "newer",
        "script-source",
        "isolated",
    ] {
        let config_path = fixture.path(&format!("native-object-rest/helpers/{config}.json"));
        if config == "broken" {
            // The pinned checker records TS2307. This tool's existing project
            // contract rejects an explicit unresolved import at load time;
            // helper checking cannot turn that missing input into a query.
            let error = Project::load(&config_path)
                .map_err(support::input_error)
                .expect_err("explicit missing helper export");
            let path = fixture.path("native-object-rest/helpers/broken.d.ts");
            let source = include_str!("../fixtures/types/native-object-rest/helpers/broken.d.ts");
            assert_eq!(error.kind, InputErrorKind::UnresolvedImport);
            assert_eq!(error.path.as_deref(), Some(path.as_path()));
            assert_eq!(
                error.position,
                Some(position(source, source.find("\"./absent\"").unwrap()))
            );
            continue;
        }
        let project = Project::load(&config_path).unwrap();
        let script = matches!(config, "script-source" | "isolated");
        let source = if script { SCRIPT } else { SOURCE };
        let file = if script { "script.ts" } else { "cases.ts" };
        for expand in [false, true] {
            let marker = "/*#assignment*/";
            let answer = project.type_of(TypeRequest {
                query: TypeQuery::At {
                    file: fixture.path(&format!("native-object-rest/helpers/{file}")),
                    position: position(source, source.find(marker).unwrap() + marker.len()),
                },
                expand,
                depth: 1,
            });
            let Ok(TypeAnswer::Selected(result)) = answer else {
                panic!("{config}/{expand}: {answer:?}")
            };
            assert_eq!(
                result.outcome,
                TypeOutcome::Found,
                "{config}/{expand}: {:?}",
                result.issues
            );
            assert_eq!(
                result.description.summary,
                "{ drop: number; kept: string; }"
            );
            assert!(result.issues.is_empty());
        }
    }
}
