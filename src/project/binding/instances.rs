//! Whether namespace declarations create runtime values: TypeScript's module
//! instance state, shared by declaration collection and the namespaces that
//! re-export names.

use super::text::{exact_text, is_text};
use crate::syntax::{child, has_modifier, id as field, ids};
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, Node, NodeId};
use std::collections::HashMap;

/// TypeScript's module instance state of a namespace body or one of its
/// statements. Combining states keeps the greatest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum Instance {
    NonInstantiated,
    /// Only const enums, which still make a namespace a value.
    ConstEnumOnly,
    Instantiated,
}

/// Whether namespace declarations create runtime values, decided as
/// TypeScript's `getModuleInstanceState` decides it. This one rule covers a
/// namespace's own body, its nested namespaces and `import =` aliases, and the
/// names its export declarations re-export.
///
/// TypeScript computes each declaration's state with a fresh cache, and a
/// node met again while its own state is being computed counts as
/// uninstantiated. A state that met such a cycle depends on where the
/// computation began, so only states that met none are kept for later
/// declarations.
#[derive(Debug, Default)]
pub(super) struct Instances {
    settled: HashMap<NodeId, Instance>,
    /// The nearest enclosing block, module block, or source file of each
    /// block and module block, built when an exported name is first looked up
    /// outside the block that exports it.
    containers: Option<HashMap<NodeId, NodeId>>,
}

/// A node whose state depends on the states of other nodes.
struct Frame {
    node: NodeId,
    /// The node whose state needed this one: for an export declaration, the
    /// module block that holds it, where its names are looked up first.
    container: NodeId,
    /// The greatest state found so far.
    state: Instance,
    /// Whether no state found so far met a cycle.
    settled: bool,
    /// The next statement of a module block or specifier of an export
    /// declaration.
    next: usize,
    lookup: Option<Lookup>,
}

/// TypeScript's `getModuleInstanceStateForAliasTarget`: the statements that
/// declare an export specifier's local name, in the innermost enclosing block
/// that has any.
struct Lookup {
    /// The local name, an identifier.
    name: NodeId,
    /// The block whose statements are being searched.
    container: NodeId,
    next: usize,
    found: Option<Instance>,
}

enum Step {
    Need(NodeId),
    Done(Instance),
}

impl Instances {
    /// A namespace declaration's instance state. One without a body is
    /// instantiated.
    pub(super) fn namespace(&mut self, ast: &Ast<'_>, namespace: NodeId) -> Instance {
        match field(ast.node(namespace), "body") {
            Some(body) => self.state(ast, body),
            None => Instance::Instantiated,
        }
    }

    /// The state of a namespace body, computed with an explicit stack.
    fn state(&mut self, ast: &Ast<'_>, root: NodeId) -> Instance {
        // This computation's states: `None` while in progress, otherwise the
        // state and whether it is settled.
        let mut visited: HashMap<NodeId, Option<(Instance, bool)>> = HashMap::new();
        let mut stack = Vec::new();
        let mut delivered = self.cached(ast, &mut visited, &mut stack, root, root);
        loop {
            let Some(frame) = stack.last_mut() else {
                return delivered.expect("the root's state is known").0;
            };
            match frame.advance(ast, delivered.take(), &mut self.containers) {
                Step::Need(node) => {
                    let container = frame.node;
                    delivered = self.cached(ast, &mut visited, &mut stack, node, container);
                }
                Step::Done(state) => {
                    let frame = stack.pop().expect("the frame just advanced");
                    visited.insert(frame.node, Some((state, frame.settled)));
                    if frame.settled {
                        self.settled.insert(frame.node, state);
                    }
                    delivered = Some((state, frame.settled));
                }
            }
        }
    }

    /// TypeScript's `getModuleInstanceStateCached`: the state of `node` and
    /// whether it is settled, or `None` after pushing a frame to compute it.
    fn cached(
        &self,
        ast: &Ast<'_>,
        visited: &mut HashMap<NodeId, Option<(Instance, bool)>>,
        stack: &mut Vec<Frame>,
        node: NodeId,
        container: NodeId,
    ) -> Option<(Instance, bool)> {
        if let Some(&state) = self.settled.get(&node) {
            return Some((state, true));
        }
        match visited.get(&node) {
            Some(None) => return Some((Instance::NonInstantiated, false)),
            Some(&Some(known)) => return Some(known),
            None => {}
        }
        if let Some(state) = leaf_state(ast, ast.node(node)) {
            return Some((state, true));
        }
        visited.insert(node, None);
        stack.push(Frame {
            node,
            container,
            state: Instance::NonInstantiated,
            settled: true,
            next: 0,
            lookup: None,
        });
        None
    }
}

impl Frame {
    /// Take the state of the node this frame needed last, if any, and return
    /// the next node it needs or its own state.
    fn advance(
        &mut self,
        ast: &Ast<'_>,
        delivered: Option<(Instance, bool)>,
        containers: &mut Option<HashMap<NodeId, NodeId>>,
    ) -> Step {
        let node = ast.node(self.node);
        let delivered = delivered.map(|(state, settled)| {
            self.settled &= settled;
            state
        });
        match node.syntax_kind {
            SyntaxKind::ModuleBlock => {
                match delivered {
                    Some(Instance::Instantiated) => return Step::Done(Instance::Instantiated),
                    Some(state) => self.state = self.state.max(state),
                    None => {}
                }
                match ids(node, "statements").get(self.next) {
                    Some(&statement) => {
                        self.next += 1;
                        Step::Need(statement)
                    }
                    None => Step::Done(self.state),
                }
            }
            SyntaxKind::ModuleDeclaration => match (delivered, field(node, "body")) {
                (Some(state), _) => Step::Done(state),
                (None, Some(body)) => Step::Need(body),
                (None, None) => unreachable!("a namespace without a body is a leaf"),
            },
            SyntaxKind::ExportDeclaration => self.export(ast, node, delivered, containers),
            kind => unreachable!("{kind:?} has a leaf state"),
        }
    }

    /// An export declaration of local names: the greatest state of the
    /// declarations each name refers to.
    fn export<'s>(
        &mut self,
        ast: &Ast<'s>,
        node: &Node<'s>,
        mut delivered: Option<Instance>,
        containers: &mut Option<HashMap<NodeId, NodeId>>,
    ) -> Step {
        let specifiers = child(ast, node, "exportClause").map_or(&[][..], |c| ids(c, "elements"));
        loop {
            if self.lookup.is_none() {
                let Some(&specifier) = specifiers.get(self.next) else {
                    return Step::Done(self.state);
                };
                self.next += 1;
                let specifier = ast.node(specifier);
                let name = field(specifier, "propertyName").or(field(specifier, "name"));
                match name.filter(|&name| ast.node(name).syntax_kind == SyntaxKind::Identifier) {
                    Some(name) => {
                        self.lookup = Some(Lookup {
                            name,
                            container: self.container,
                            next: 0,
                            found: None,
                        });
                    }
                    // A string name is invalid here; TypeScript assumes a value.
                    None => return Step::Done(Instance::Instantiated),
                }
            }
            let lookup = self.lookup.as_mut().expect("a lookup is in progress");
            match lookup.advance(ast, delivered.take(), containers) {
                Step::Need(statement) => return Step::Need(statement),
                Step::Done(state) => {
                    self.lookup = None;
                    self.state = self.state.max(state);
                    if self.state == Instance::Instantiated {
                        return Step::Done(Instance::Instantiated);
                    }
                }
            }
        }
    }
}

impl Lookup {
    /// Take the state of the statement needed last, if any, and return the
    /// next declaration of the name or the name's state.
    fn advance(
        &mut self,
        ast: &Ast<'_>,
        delivered: Option<Instance>,
        containers: &mut Option<HashMap<NodeId, NodeId>>,
    ) -> Step {
        let name = exact_text(ast.node(self.name)).unwrap_or_default();
        if let Some(state) = delivered {
            let found = self.found.map_or(state, |found| found.max(state));
            if found == Instance::Instantiated {
                return Step::Done(found);
            }
            // TypeScript counts a re-exported `import =` alias as a value,
            // since its target may be one.
            let previous = ids(ast.node(self.container), "statements")[self.next - 1];
            self.found = Some(
                if ast.node(previous).syntax_kind == SyntaxKind::ImportEqualsDeclaration {
                    Instance::Instantiated
                } else {
                    found
                },
            );
        }
        loop {
            let statements = ids(ast.node(self.container), "statements");
            while let Some(&statement) = statements.get(self.next) {
                self.next += 1;
                if has_name(ast, ast.node(statement), name) {
                    return Step::Need(statement);
                }
            }
            if let Some(found) = self.found {
                return Step::Done(found);
            }
            let containers = containers.get_or_insert_with(|| enclosing_containers(ast));
            match containers.get(&self.container) {
                Some(&outer) => {
                    self.container = outer;
                    self.next = 0;
                }
                // Declared in no enclosing block; TypeScript assumes a value.
                None => return Step::Done(Instance::Instantiated),
            }
        }
    }
}

/// The state of a node that depends on no other node's state, as
/// TypeScript's `getModuleInstanceStateWorker` decides it. A module block, a
/// namespace with a body, and an export declaration of local names have none.
fn leaf_state<'s>(ast: &Ast<'s>, node: &Node<'s>) -> Option<Instance> {
    Some(match node.syntax_kind {
        SyntaxKind::InterfaceDeclaration | SyntaxKind::TypeAliasDeclaration => {
            Instance::NonInstantiated
        }
        SyntaxKind::EnumDeclaration if has_modifier(ast, node, SyntaxKind::ConstKeyword) => {
            Instance::ConstEnumOnly
        }
        SyntaxKind::ImportDeclaration | SyntaxKind::ImportEqualsDeclaration
            if !has_modifier(ast, node, SyntaxKind::ExportKeyword) =>
        {
            Instance::NonInstantiated
        }
        SyntaxKind::ModuleBlock => return None,
        SyntaxKind::ModuleDeclaration if field(node, "body").is_some() => return None,
        SyntaxKind::ExportDeclaration
            if field(node, "moduleSpecifier").is_none()
                && child(ast, node, "exportClause")
                    .is_some_and(|clause| clause.syntax_kind == SyntaxKind::NamedExports) =>
        {
            return None;
        }
        _ => Instance::Instantiated,
    })
}

/// TypeScript's `nodeHasName`: whether a statement's own name, or the name of
/// one of its variables, is the identifier `name`.
fn has_name<'s>(ast: &Ast<'s>, statement: &Node<'s>, name: &str) -> bool {
    let named = |node: &Node<'s>| match child(ast, node, "name") {
        Some(own) => own.syntax_kind == SyntaxKind::Identifier && is_text(own, name),
        None => false,
    };
    if field(statement, "name").is_some() {
        return named(statement);
    }
    statement.syntax_kind == SyntaxKind::VariableStatement
        && child(ast, statement, "declarationList").is_some_and(|list| {
            ids(list, "declarations")
                .iter()
                .any(|&declaration| named(ast.node(declaration)))
        })
}

/// The nearest enclosing block, module block, or source file of each block
/// and module block reachable from the root.
fn enclosing_containers(ast: &Ast<'_>) -> HashMap<NodeId, NodeId> {
    let root = ast.root_id();
    let mut containers = HashMap::new();
    let mut stack = vec![(root, root)];
    while let Some((id, container)) = stack.pop() {
        let node = ast.node(id);
        let inner = if matches!(
            node.syntax_kind,
            SyntaxKind::Block | SyntaxKind::ModuleBlock
        ) {
            containers.insert(id, container);
            id
        } else {
            container
        };
        stack.extend(ast.children(id).map(|child| (child, inner)));
    }
    containers
}
