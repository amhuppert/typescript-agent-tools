//! Native couldContainTypeVariables recursive and flat final-query limits.
use super::*;

#[test]
fn request_resource_containment_depth() {
    for strict_null_checks in [true, false] {
        for no_check in [false, true] {
            for source in [
                include_str!(
                    "../../../../fixtures/types/native-containment-resources/deep-small.ts"
                ),
                include_str!(
                    "../../../../fixtures/types/native-containment-resources/deep-wide.ts"
                ),
            ] {
                let fixture =
                    Fixture::with_null_checks(source, "deepChosen", no_check, strict_null_checks);
                let usage = healthy(fixture.query(Limits::default()));
                interrupted(
                    fixture.query(Limits {
                        depth: Some(usage.peak_depth - 1),
                    }),
                    SemanticErrorKind::SemanticDepthLimit,
                    SemanticResourceReason::SemanticDepthLimit,
                );
                for depth in [usage.peak_depth, usage.peak_depth + 1] {
                    healthy(fixture.query(Limits { depth: Some(depth) }));
                }
                // These are whole-query depth boundaries; the visit test
                // independently detects omission of the scoped entry guard.
                eprintln!(
                    "containment depth {strict_null_checks}/{no_check}: {}",
                    usage.peak_depth
                );
            }
        }
    }
}
