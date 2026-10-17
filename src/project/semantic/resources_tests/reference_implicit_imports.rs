//! Synthesized helper imports preserve native groups.
const FILES: &[(&str, &str)] = &[
    (
        "globals.d.ts",
        include_str!("../../../../fixtures/types/native-implicit-import-resources/globals.d.ts"),
    ),
    (
        "factory.ts",
        include_str!("../../../../fixtures/types/native-implicit-import-resources/factory.ts"),
    ),
    (
        "default.ts",
        include_str!("../../../../fixtures/types/native-implicit-import-resources/default.ts"),
    ),
    (
        "types.ts",
        include_str!("../../../../fixtures/types/native-implicit-import-resources/types.ts"),
    ),
    (
        "ambient.d.ts",
        include_str!("../../../../fixtures/types/native-implicit-import-resources/ambient.d.ts"),
    ),
    (
        "ambient-use.ts",
        include_str!("../../../../fixtures/types/native-implicit-import-resources/ambient-use.ts"),
    ),
    (
        "global-module.d.ts",
        include_str!(
            "../../../../fixtures/types/native-implicit-import-resources/global-module.d.ts"
        ),
    ),
    (
        "global-use.ts",
        include_str!("../../../../fixtures/types/native-implicit-import-resources/global-use.ts"),
    ),
];
fn configuration(profile: &str) -> &'static str {
    match profile {
        "strict" => {
            include_str!("../../../../fixtures/types/native-implicit-import-resources/strict.json")
        }
        "nocheck" => {
            include_str!("../../../../fixtures/types/native-implicit-import-resources/nocheck.json")
        }
        "loose" => {
            include_str!("../../../../fixtures/types/native-implicit-import-resources/loose.json")
        }
        "loose-nocheck" => include_str!(
            "../../../../fixtures/types/native-implicit-import-resources/loose-nocheck.json"
        ),
        _ => unreachable!(),
    }
}

#[test]
fn request_resource_implicit_native_branch_outcomes() {
    super::reference_import_branches::check_matrix(super::reference_import_branches::Matrix {
        files: FILES,
        expected: include_str!(
            "../../../../fixtures/types/native-implicit-import-resources/expected.json"
        ),
        configuration,
    });
}
