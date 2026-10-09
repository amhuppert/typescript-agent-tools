//! Lexical binding rules that the semantic oracle cannot check. The marked
//! binding cases live in `fixtures/references`, where both the oracle and
//! `tests/reference_fixtures.rs` check them.
mod support;

use support::load_error;
use typescript_parser::analysis::InputErrorKind;

/// docs/references.md, "Differences from TypeScript 7.0.2": invalid
/// declaration arrangements that TypeScript binds but its checker rejects are
/// `Binding` errors here, so these projects have no results to compare.
#[test]
fn invalid_declaration_arrangements_are_binding_errors() {
    for source in [
        "export {};\nfunction f() {}\nfunction f() {}\n",
        "export {};\nclass C {}\nfunction C() {}\n",
        "export {};\nfunction C() {}\nclass C {}\n",
        "export {};\ntype F<T, T> = T;\n",
        "export {};\nfunction f() { { let x = 1; { var x = 2; } } }\n",
        "export {};\nnamespace N { export interface X {} interface X {} }\n",
        "export interface X {}\ninterface X {}\n",
        "export {};\nnamespace f { export const x = 1; }\nfunction f() {}\n",
        "export {};\ninterface A<T, U> {}\ninterface A<U, T> {}\n",
        "export {};\ninterface B<T> {}\ninterface B {}\n",
        "export {};\ninterface E<T, U> {}\ninterface E<T> {}\n",
        "export {};\ninterface G<T> {}\ninterface G<T, U> {}\n",
        "export {};\ninterface H<T, U = 1> {}\ninterface H<T, V = 1> {}\n",
    ] {
        let error = load_error(&[("app.ts", source)]);
        assert_eq!(error.kind, InputErrorKind::Binding, "{source}: {error}");
    }
}

/// A namespace whose body creates a value is a value, which a constant of the
/// same name conflicts with (TypeScript's TS2300). The uninstantiated forms
/// live in the `namespace-instantiation` fixture, which the oracle checks.
#[test]
fn instantiated_namespaces_conflict_with_values() {
    for body in [
        // An exported namespace that declares a value.
        "namespace B { export const x: number; } export { B };",
        // A namespace that is also a value.
        "namespace B { type T = 1; } const B: number; export { B };",
        // A re-exported `import =` might name a value.
        "namespace B { type T = 1; } import x = B; export { x };",
        "namespace B { type T = 1; } export import x = B;",
        // A const enum.
        "const enum E { a } export { E };",
        // A nested namespace that declares a value, exported or not.
        "namespace B { const x: number; }",
        // An exported name declared nowhere might be a value.
        "export { Missing };",
    ] {
        let source = format!("declare namespace A {{ {body} }}\ndeclare const A: number;\n");
        let error = load_error(&[("app.ts", &source)]);
        assert_eq!(error.kind, InputErrorKind::Binding, "{source}: {error}");
        assert_eq!(error.message, "duplicate declaration of `A`", "{source}");
    }
}
