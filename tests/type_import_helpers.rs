//! Native synthetic helper dependencies affect final type queries, without
//! turning a missing helper into an unrelated project-load failure.
mod support;
use serde_json::{Value, json};
use support::{Fixture, position};
use typescript_parser::analysis::{AnalysisError, SemanticErrorKind};
use typescript_parser::types::{TypeAnswer, TypeOutcome, TypeQuery, TypeRequest};

#[test]
fn helper_imports_follow_effective_module_policy_and_missing_helpers_stay_cold() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../fixtures/types/native-import-helpers/cases.json"
    ))
    .unwrap();
    for case in cases {
        let file = case["file"].as_str().unwrap();
        let source = case["source"].as_str().unwrap();
        let mut options = json!({"noLib":true,"types":[],"moduleDetection":"legacy","paths":{"tslib":["./helpers.d.ts"]}});
        options
            .as_object_mut()
            .unwrap()
            .extend(case["options"].as_object().unwrap().clone());
        let config = json!({"compilerOptions":options,"files":[file,"globals.d.ts"]}).to_string();
        let fixture = Fixture::new(&[
            ("tsconfig.json", &config),
            (file, source),
            (
                "helpers.d.ts",
                include_str!("../fixtures/types/native-import-helpers/helpers.d.ts"),
            ),
            (
                "globals.d.ts",
                include_str!("../fixtures/types/native-import-helpers/globals.d.ts"),
            ),
        ]);
        if file.ends_with(".json") {
            fixture.load();
            std::fs::write(
                fixture.path("helpers.d.ts"),
                "export declare const broken: ;",
            )
            .unwrap();
            let loaded = typescript_parser::project::Project::load(&fixture.path("tsconfig.json"))
                .map_err(support::input_error);
            if case["loaded"] == true {
                assert!(
                    matches!(loaded,Err(error) if error.kind == typescript_parser::analysis::InputErrorKind::Syntax),
                    "{}",
                    case["name"]
                );
            } else {
                assert!(loaded.is_ok(), "{}: {loaded:?}", case["name"]);
            }
            continue;
        }
        let answer = fixture.load().type_of(TypeRequest::new(TypeQuery::At {
            file: fixture.path(file),
            position: position(source, source.find("selected:").unwrap()),
        }));
        if case["loaded"] == true {
            assert!(
                matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found && result.description.summary == "27"),
                "{}: {answer:?}",
                case["name"]
            );
        } else {
            assert!(
                matches!(&answer, Err(AnalysisError::Semantic(error)) if error.kind == SemanticErrorKind::InvalidSemantic),
                "{}: {answer:?}",
                case["name"]
            );
        }
        std::fs::remove_file(fixture.path("helpers.d.ts")).unwrap();
        let healthy = format!(
            "{}declare let /*healthy*/ healthy: number;\n",
            if source.starts_with("export") {
                "export {};\n"
            } else {
                ""
            }
        );
        std::fs::write(fixture.path(file), &healthy).unwrap();
        options["noUncheckedSideEffectImports"] = true.into();
        std::fs::write(
            fixture.path("tsconfig.json"),
            json!({"compilerOptions":options,"files":[file,"globals.d.ts"]}).to_string(),
        )
        .unwrap();
        let answer = fixture.load().type_of(TypeRequest::new(TypeQuery::At {
            file: fixture.path(file),
            position: position(&healthy, healthy.find("healthy:").unwrap()),
        }));
        assert!(
            matches!(&answer, Ok(TypeAnswer::Selected(result)) if result.outcome == TypeOutcome::Found && result.description.summary == "number"),
            "{} missing: {answer:?}",
            case["name"]
        );
    }
}
