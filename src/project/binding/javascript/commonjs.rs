//! A JavaScript file as a CommonJS module (docs/references.md, JavaScript
//! inputs). TypeScript 7.0.2's binder sets a file's CommonJS module
//! indicator at its first CommonJS export or `require` call where no ES
//! module syntax makes it a module (`setCommonJSModuleIndicator`), which
//! makes the file a module (`bindSourceFileAsExternalModule`) with `module`
//! and `exports` variables of its own (`declareCommonJSVariable`), and binds
//! `module.exports = name` as the module's `export=` alias, whose value
//! declaration is the assignment (`bindModuleExportsAssignment`). This tool
//! binds that one form (`module_exports`) and refuses every other CommonJS
//! export (`super::refusals`); 5.6.3's `setCommonJsModuleIndicator` and
//! `bindModuleExportsAssignment` decide it as 7.0.2 does, but where the
//! name is an alias of `exports` (`isExportsOrModuleExportsOrAlias`), which
//! that dialect refuses (`super::typescript_5_6`). The indicator follows
//! 7.0.2's classification in both dialects: where 5.6.3's differs, as for
//! `exports.a = void 0`, which leaves a script, the export is refused in both
//! (`super::Rules`), so only the bound form makes a loaded file a module.

use crate::project::binding::ModuleIndicators;
use crate::syntax::{self, AssignmentDeclarationKind, id, ids};
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, NodeId};

/// What decides a JavaScript file's CommonJS exports: whether ES module
/// syntax makes it a module (its `ExternalModuleIndicator`, which
/// `setCommonJSModuleIndicator` reads), and the `module.exports` assignment
/// binding admits (`module_exports`).
#[derive(Debug, Clone, Copy)]
pub(in crate::project::binding) struct Exports {
    pub(in crate::project::binding) module_syntax: bool,
    pub(in crate::project::binding) admitted: Option<NodeId>,
}

/// The variables `declareCommonJSVariable` declares in a CommonJS module,
/// where its own top-level declarations do not.
pub(in crate::project::binding) const VARIABLES: [&str; 2] = ["module", "exports"];

/// Whether a JavaScript file has a node at which `setCommonJSModuleIndicator`
/// is asked: an assignment declaring a CommonJS export (`bindModuleExportsAssignment`,
/// `bindExportsOrObjectDefineProperty`) or a `require` call
/// (`bindCallExpression`). It succeeds, making the file a module, where
/// neither ES module syntax nor a JSX element under `jsx: react-jsx`
/// already does (`ModuleIndicators::module`).
pub(in crate::project::binding) fn has_indicator(ast: &Ast<'_>) -> bool {
    crate::javascript_syntax::is_javascript(ast)
        && ast.walk().any(|node| {
            matches!(
                syntax::assignment_declaration_kind(ast, node, true),
                AssignmentDeclarationKind::ModuleExports
                    | AssignmentDeclarationKind::ExportsProperty
                    | AssignmentDeclarationKind::ObjectDefinePropertyExports
            ) || syntax::is_require_call(ast, node, false)
        })
}

/// The `module.exports = name` this tool binds as the module's `export=`
/// alias (`bindModuleExportsAssignment`, `ExpressionIsAlias`): the file's only
/// `module.exports` assignment (`JSDeclarationKindModuleExports`), a
/// top-level statement whose operand is an identifier, in a file whose
/// CommonJS indicator is set, so without ES module syntax. Deliberately
/// broader than TypeScript, a file with a JSX element has none, as under
/// `jsx: react-jsx` the element, not the assignment, makes it a module.
pub(in crate::project::binding) fn module_exports(
    ast: &Ast<'_>,
    indicators: ModuleIndicators,
) -> Option<NodeId> {
    if !indicators.commonjs || indicators.external || indicators.first_jsx.is_some() {
        return None;
    }
    let mut assignments = ast.walk().filter(|&node| {
        syntax::assignment_declaration_kind(ast, node, true)
            == AssignmentDeclarationKind::ModuleExports
    });
    let assignment = assignments.next()?;
    if assignments.next().is_some() {
        return None;
    }
    let top_level = ids(ast.root(), "statements").iter().any(|&statement| {
        let statement = ast.node(statement);
        statement.syntax_kind == SyntaxKind::ExpressionStatement
            && id(statement, "expression") == Some(assignment)
    });
    let named = id(ast.node(assignment), "right")
        .is_some_and(|right| ast.node(right).syntax_kind == SyntaxKind::Identifier);
    (top_level && named).then_some(assignment)
}
