//! Prewarmed recursive mapping application versus a long shallow control.
use super::*;

fn query_case(
    row: &serde_json::Value,
    limits: Limits,
) -> Result<(TypeAnswer, crate::resources::Usage), AnalysisError> {
    let v = &row["variant"];
    let file = v["file"].as_str().unwrap();
    let mut fixture = Fixture::new("let chosen: number = 1;");
    std::fs::write(
        fixture.directory.join("globals.d.ts"),
        include_str!("../../../../fixtures/types/native-mapping-depth-resources/globals.d.ts"),
    )
    .unwrap();
    std::fs::write(
        fixture.directory.join(file),
        row["source"].as_str().unwrap(),
    )
    .unwrap();
    std::fs::write(
        fixture.directory.join("tsconfig.json"),
        v["configuration"].to_string(),
    )
    .unwrap();
    fixture.project = Project::load(&fixture.directory.join("tsconfig.json")).unwrap();
    let request = TypeRequest::new(TypeQuery::At {
        file: fixture.directory.join(file),
        position: SourcePosition {
            line: row["at"]["line"].as_u64().unwrap() as usize,
            column: row["at"]["column"].as_u64().unwrap() as usize,
        },
    });
    let answer = worker_with_limits(&fixture.project, limits, |parser, control| {
        let answer = type_of_here(&fixture.project, &request, parser, control.clone())?;
        Ok((answer, control.usage()))
    });
    let source = row["source"].as_str().unwrap();
    let healthy_request = TypeRequest::new(TypeQuery::At {
        file: fixture.directory.join(file),
        position: crate::source::SourceText::new(source.to_owned())
            .position(source.find("healthy").unwrap()),
    });
    healthy(fixture.query_request(&healthy_request, Limits::default()));
    answer
}

fn verify(row: &serde_json::Value, answer: TypeAnswer) {
    let TypeAnswer::Selected(answer) = answer else {
        panic!("unselected: {}", row["variant"]);
    };
    assert_eq!(
        answer.outcome,
        TypeOutcome::Found,
        "{}: {answer:?}",
        row["variant"]
    );
    assert_eq!(
        answer.description.summary,
        row["answer"]["summary"].as_str().unwrap(),
        "{}",
        row["variant"]
    );
    assert!(answer.issues.is_empty(), "{}: {answer:?}", row["variant"]);
}
fn rows() -> Vec<serde_json::Value> {
    let mut rows: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../../fixtures/types/native-mapping-depth-resources/expected.json"
    ))
    .unwrap();
    assert_eq!(rows.len(), 64);
    let extra: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../../fixtures/types/native-mapping-depth-resources/flat-warmup/expected.json"
    ))
    .unwrap();
    assert_eq!(extra.len(), 64);
    rows.extend(extra);
    rows
}
#[test]
fn request_resource_mapping_depth_native_values() {
    for row in rows() {
        let (answer, usage) = query_case(&row, Limits::default()).unwrap();
        verify(&row, answer);
        assert_eq!(usage.depth, 0);
    }
}
#[test]
fn request_resource_mapping_application_depth() {
    let rows = rows();
    let mut executed = 0;
    for nested in rows
        .iter()
        .filter(|r| r["variant"]["layout"] == "nested" && r["variant"]["warmup"] == "array")
    {
        executed += 1;
        let v = &nested["variant"];
        let flat = rows
            .iter()
            .find(|r| {
                r["variant"]["layout"] == "flat"
                    && r["variant"]["warmup"] == "array"
                    && ["width", "outer", "matches", "profile"]
                        .iter()
                        .all(|key| r["variant"][*key] == v[*key])
            })
            .unwrap();
        let (answer, flat_usage) = query_case(flat, Limits::default()).unwrap();
        verify(flat, answer);
        let cap = flat_usage.peak_depth;
        let limited = |depth| Limits { depth: Some(depth) };
        verify(flat, query_case(flat, limited(cap)).unwrap().0);
        let (answer, usage) = query_case(nested, Limits::default()).unwrap();
        verify(nested, answer);
        assert_eq!(usage.depth, 0);
        // The 16-layer application stays beneath the startup/warmup peak.
        // The wide case must expose actual descent beyond the same flat input.
        if v["width"] == 64 {
            let restricted = query_case(nested, limited(cap));
            eprintln!(
                "mapping depth case {v}: flat {}, nested {}, cap {cap}",
                flat_usage.peak_depth, usage.peak_depth
            );
            match &restricted {
                Ok((answer, usage)) => {
                    eprintln!("restricted success: {answer:?}, depth {}", usage.peak_depth)
                }
                Err(error) => eprintln!("restricted error: {error:?}"),
            }
            assert_interrupted(
                restricted,
                SemanticErrorKind::SemanticDepthLimit,
                SemanticResourceReason::SemanticDepthLimit,
            );
        }
        assert_interrupted(
            query_case(nested, limited(usage.peak_depth - 1)),
            SemanticErrorKind::SemanticDepthLimit,
            SemanticResourceReason::SemanticDepthLimit,
        );
        for depth in [usage.peak_depth, usage.peak_depth + 1] {
            verify(nested, query_case(nested, limited(depth)).unwrap().0);
        }
    }
    assert_eq!(executed, 32);
}
