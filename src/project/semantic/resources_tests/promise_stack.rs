//! Native warmed Promise chains isolate awaited recursion from source depth.
use super::*;

const SMALL: &str = concat!(
    include_str!("../../../../fixtures/types/native-promise-stack-resources/globals.d.ts"),
    "\n",
    include_str!("../../../../fixtures/types/native-promise-stack-resources/small.ts")
);
const DEEP: &str = concat!(
    include_str!("../../../../fixtures/types/native-promise-stack-resources/globals.d.ts"),
    "\n",
    include_str!("../../../../fixtures/types/native-promise-stack-resources/deep.ts")
);

const UNION_SMALL: &str = concat!(
    include_str!("../../../../fixtures/types/native-promise-stack-resources/globals.d.ts"),
    "\n",
    include_str!("../../../../fixtures/types/native-promise-stack-resources/union-small.ts")
);
const UNION_DEEP: &str = concat!(
    include_str!("../../../../fixtures/types/native-promise-stack-resources/globals.d.ts"),
    "\n",
    include_str!("../../../../fixtures/types/native-promise-stack-resources/union-deep.ts")
);
const CASES: &[(&str, u64, bool)] = &[
    (SMALL, 8, false),
    (DEEP, 32, false),
    (UNION_SMALL, 4, true),
    (UNION_DEEP, 16, true),
];
const FLAT_SMALL: &str = concat!(
    include_str!("../../../../fixtures/types/native-promise-cache-resources/globals.d.ts"),
    "\n",
    include_str!("../../../../fixtures/types/native-promise-cache-resources/small.ts")
);
const FLAT_WIDE: &str = concat!(
    include_str!("../../../../fixtures/types/native-promise-cache-resources/globals.d.ts"),
    "\n",
    include_str!("../../../../fixtures/types/native-promise-cache-resources/wide.ts")
);

#[test]
fn request_resource_promise_recursion_depth() {
    for strict_null_checks in [true, false] {
        for no_check in [false, true] {
            for &(source, layers, union) in CASES {
                let depth = if union { 2 * layers } else { layers } as usize;
                let fixture = Fixture::with_target(
                    source,
                    "chosen",
                    no_check,
                    strict_null_checks,
                    Some("ESNext"),
                );
                let usage = healthy(fixture.query(Limits::default()));
                assert!(
                    usage.peak_depth > depth,
                    "awaited native depth {depth}: checked peak {}",
                    usage.peak_depth
                );
                for limit in [depth, usage.peak_depth - 1] {
                    interrupted(
                        fixture.query(Limits { depth: Some(limit) }),
                        SemanticErrorKind::SemanticDepthLimit,
                        SemanticResourceReason::SemanticDepthLimit,
                    );
                }
                for limit in [usage.peak_depth, usage.peak_depth + 1] {
                    healthy(fixture.query(Limits { depth: Some(limit) }));
                }
                healthy_successor(&fixture, source);
            }
            let mut shallow = None;
            for source in [FLAT_SMALL, FLAT_WIDE] {
                let fixture = Fixture::with_target(
                    source,
                    "chosen",
                    no_check,
                    strict_null_checks,
                    Some("ESNext"),
                );
                let usage = healthy(fixture.query(Limits { depth: shallow }));
                if let Some(depth) = shallow {
                    assert_eq!(usage.peak_depth, depth);
                } else {
                    shallow = Some(usage.peak_depth);
                }
                healthy_successor(&fixture, source);
            }
        }
    }
}
