//! Request-local syntax access over captured inputs. Parser arena links never
//! become semantic identities, and evaluation never reopens a source file.

mod access;
pub(super) mod arrays;
mod assignments;
mod before_use;
mod calls;
mod capture;
pub(super) mod class_checks;
pub(super) mod conditional;
pub(super) mod constants;
mod deprecated;
mod effects;
pub(super) mod elaboration;
mod emit_helpers;
pub(super) mod enums;
mod functions;
mod generics;
pub(super) mod identifiers;
mod import_types;
mod instantiation_expressions;
mod iteration;
mod javascript;
pub(super) mod mapped;
mod members;
mod module_specifiers;
mod narrowing;
mod native_display;
mod node_types;
pub(super) mod object_literals;
pub(super) use node_types::NodeTypeForm;
pub(super) mod operators;
mod overloads;
mod predicates;
mod property_names;
pub(super) mod pseudo_types;
mod reads;
mod references;
pub(super) use references::{
    EntitySymbolMissing, ReferenceSite, ReferenceSiteTarget, ReferenceSymbolRecipe,
};
mod reference_search;
pub(super) use reference_search::ReferenceMeaning;
pub(super) mod reference_import_tracker;
mod reference_imports;
pub(super) use reference_imports::{
    ReferenceExportKind, ReferenceExportSpecifier, ReferenceExportSyntax,
};
mod relations;
mod returns;
mod super_expression;
mod switches;
pub(super) mod symbol_accessibility;
pub(super) mod symbol_aliases;
pub(super) mod symbol_visibility;
mod symbols;
pub(super) mod templates;
mod this;
mod tuples;
pub(super) use super_expression::SuperExpressionContext;
pub(super) mod type_checks;
mod unique_symbols;
pub(super) mod variables;
pub(super) use access::{AccessKey, AccessRecipe, ElementAccessName};
pub(super) use assignments::{AssignmentProjection, AssignmentTypeRecipe};
pub(super) use calls::{CallRecipe, CallRoot, CallTypeArgument};
pub(super) use capture::AssignmentSite;
pub(super) use effects::{DottedName, ExplicitDeclarationType};
pub(super) use functions::{ContextualThisSource, FunctionExpressionFacts};
pub(super) use generics::{
    ClassRecipe, ParameterConstraintRecipe, ThisTypeContext, TypeParameterRecipe,
};
pub(super) use import_types::ImportTypeRecipe;
pub(super) use instantiation_expressions::InstantiationSite;
pub(super) use members::{
    AccessDeclarationContext, DeclarationIdentity, DeclarationReference, OuterParameterRecipe,
    ParameterReferenceSyntax,
};
pub(super) use narrowing::{ConstraintPosition, ContextRecipe, ReferencePosition};
pub(super) use overloads::SignatureParent;
pub(super) use property_names::{EffectivePropertyName, PropertyNameForm};
pub(super) use reads::{FlowSyntax, VariableReadFacts};
pub(super) use returns::{ReturnBodyKind, ReturnBodyRecipe, ReturnExpressionRecipe};
pub(super) use tuples::{
    AliasResolutionRecipe, ArrayOrTupleRecipe, DeferredReferenceRecipe, TupleElementRecipe,
};

use super::super::binding::BoundDeclaration;
use super::super::sources::FileId;
use super::inputs::CheckerInputs;
use super::model::{LiteralValue, SignatureFlags, SpecialName};
use crate::analysis::{AnalysisError, InputError, Parser, SemanticFailureReason};
use crate::references::Location;
use crate::syntax;
use crate::syntax_kind::SyntaxKind;
use crate::types::{PrimitiveName, TypePredicateKind};
use crate::{Ast, NodeId, Span, Value};
use std::collections::HashMap;
mod unresolved;

pub(super) enum SelectedSyntax {
    Alias {
        declaration: DeclarationIdentity,
        type_only: bool,
        token: Span,
    },
    Declaration {
        anchor: Span,
        kind: DeclarationKind,
        declaration: DeclarationIdentity,
    },
    Member {
        anchor: Span,
        declaration: DeclarationIdentity,
    },
    Type(TypeExpression),
    ValueAt(Expression),
    Expression(Expression),
    PendingValue {
        root: MemberRoot,
        name: Option<String>,
        reason: SemanticFailureReason,
        message: String,
    },
    UnsupportedTarget {
        token: Span,
    },
    Pending {
        token: Span,
    },
}

/// The exact own source root of a pending PropertyAssignment. Its raw role
/// survives selection without inventing a canonical semantic member identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct MemberRoot {
    pub(super) file: FileId,
    start: usize,
    end: usize,
    role: MemberRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum MemberRole {
    PropertyAssignment,
}

impl MemberRoot {
    pub(super) fn span(self) -> Span {
        Span {
            start: self.start,
            end: self.end,
        }
    }
}

pub(super) enum DeclarationKind {
    Variable,
    Function,
    TypeAlias,
    Class,
    Interface,
    TypeParameter,
    Enum,
    EnumMember,
}

/// Immutable syntax recipe for `getTypeFromTypeNode`.
#[derive(Clone)]
pub(super) struct TypeExpression(std::sync::Arc<TypeExpressionData>);

impl std::ops::Deref for TypeExpression {
    type Target = TypeExpressionData;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TypeExpression {
    fn new(data: TypeExpressionData) -> Self {
        Self(std::sync::Arc::new(data))
    }
}

pub(super) struct TypeExpressionData {
    pub(super) file: FileId,
    pub(super) root: Option<ExpressionRoot>,
    pub(super) location: Location,
    pub(super) form: TypeExpressionForm,
}

/// Source inputs for native alias identity and its separate written name.
/// Evaluation acquires the canonical symbol of this actual declaration node
/// (`getSymbolOfDeclaration`) before establishing a Model Alias.
#[derive(Clone)]
pub(super) struct AliasRecipe {
    pub(super) declaration: DeclarationIdentity,
}

pub(super) enum TypeExpressionForm {
    Unique(ExpressionRoot),
    Mapped(ExpressionRoot),
    Conditional(ExpressionRoot),
    Infer(ExpressionRoot),
    Primitive(PrimitiveName),
    Special(SpecialName),
    TemplateLiteral(ExpressionRoot),
    IntrinsicMarker,
    Literal(Expression),
    This(generics::ThisTypeContext),
    Query(ExpressionRoot),
    /// `getTypeFromImportTypeNode`; its recipe is acquired when evaluated.
    Import(ExpressionRoot),
    ArrayOrTuple(ArrayOrTupleRecipe),
    TupleElement(TupleElementRecipe),
    Readonly {
        site: super::super::binding::flow::Site,
        value: Box<TypeExpression>,
    },
    TypeLiteral {
        anchor: Span,
        alias: Option<AliasRecipe>,
    },
    Union {
        site: super::super::binding::flow::Site,
        types: Vec<TypeExpression>,
        alias: Option<AliasRecipe>,
    },
    Intersection {
        site: super::super::binding::flow::Site,
        types: Vec<TypeExpression>,
        alias: Option<AliasRecipe>,
    },
    Keyof {
        site: super::super::binding::flow::Site,
        value: Box<TypeExpression>,
    },
    IndexedAccess {
        site: super::super::binding::flow::Site,
        object: Box<TypeExpression>,
        index: Box<TypeExpression>,
        alias: Option<AliasRecipe>,
    },
    Reference {
        site: super::super::binding::flow::Site,
        anchor: Option<Span>,
        arguments: Vec<TypeExpression>,
        alias: Option<AliasRecipe>,
        deferred: Option<DeferredReferenceRecipe>,
    },
    Pending {
        reason: SemanticFailureReason,
        message: String,
    },
}

#[derive(Clone)]
pub(super) struct IndexSignatureRecipe {
    pub(super) declaration: DeclarationIdentity,
    pub(super) parameter_name: String,
    pub(super) key_type: TypeExpression,
    pub(super) value_type: Option<TypeExpression>,
    pub(super) readonly: bool,
}

/// An actual own signature declaration, not a member or a display identity.
/// Instantiated/composite signatures require their native target/mapper owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct SignatureRoot {
    pub(super) file: FileId,
    start: usize,
    end: usize,
    role: SignatureRole,
}
// Source signature attachment identity contains only fixed file/span/role
// fields; hashing and equality cannot visit text or allocate key payload.

impl SignatureRoot {
    pub(super) fn span(self) -> Span {
        Span {
            start: self.start,
            end: self.end,
        }
    }

    pub(super) fn site(self) -> super::super::binding::flow::Site {
        super::super::binding::flow::Site {
            span: self.span(),
            kind: match self.role {
                SignatureRole::FunctionDeclaration => SyntaxKind::FunctionDeclaration,
                SignatureRole::FunctionExpression => SyntaxKind::FunctionExpression,
                SignatureRole::ArrowFunction => SyntaxKind::ArrowFunction,
                SignatureRole::FunctionType => SyntaxKind::FunctionType,
                SignatureRole::ConstructorType => SyntaxKind::ConstructorType,
                SignatureRole::Constructor => SyntaxKind::Constructor,
                SignatureRole::Call => SyntaxKind::CallSignature,
                SignatureRole::Construct => SyntaxKind::ConstructSignature,
                SignatureRole::Index => SyntaxKind::IndexSignature,
                SignatureRole::MethodDeclaration => SyntaxKind::MethodDeclaration,
                SignatureRole::MethodSignature => SyntaxKind::MethodSignature,
                SignatureRole::Getter => SyntaxKind::GetAccessor,
                SignatureRole::Setter => SyntaxKind::SetAccessor,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SignatureRole {
    FunctionDeclaration,
    FunctionExpression,
    ArrowFunction,
    FunctionType,
    ConstructorType,
    Constructor,
    Call,
    Construct,
    Index,
    MethodDeclaration,
    MethodSignature,
    Getter,
    Setter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum SignatureQuestion {
    This,
    ThisDisplay,
    ThisCall,
    Parameter(usize),
    ParameterDisplay(usize),
    ParameterCall(usize),
    Return,
    Predicate,
}

#[derive(Clone)]
pub(super) struct SignatureRecipe {
    pub(super) root: SignatureRoot,
    /// Actual TypeParameter declaration nodes; `getTypeParametersFromDeclaration`
    /// acquires their symbols, never a spelling environment.
    pub(super) type_parameters: Vec<DeclarationIdentity>,
    pub(super) this_parameter: Option<SignatureParameterRecipe>,
    pub(super) parameters: Vec<SignatureParameterRecipe>,
    pub(super) return_type: SignatureReturnRecipe,
    pub(super) predicate: Option<SignaturePredicateRecipe>,
    pub(super) origin: Location,
    pub(super) flags: SignatureFlags,
    pub(super) untyped_javascript: UntypedJavaScript,
}

/// The syntax `getSignatureFromDeclaration` reads for
/// `IsUntypedSignatureInJSFile`: a function, method, accessor, or
/// constructor of a JavaScript file, not immediately invoked, with no
/// parameter annotated. Its last term, that the declaration has no
/// contextual type, is the evaluator's.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum UntypedJavaScript {
    No,
    /// A declaration, which no expression gives a contextual type.
    Declaration,
    /// A function expression, arrow function, or object literal method or
    /// accessor, untyped where it has no contextual type.
    Expression,
}

#[derive(Clone)]
pub(super) struct SignaturePredicateRecipe {
    pub(super) kind: TypePredicateKind,
    pub(super) subject: SignaturePredicateSubject,
    pub(super) value_type: Option<TypeExpression>,
}

#[derive(Clone)]
pub(super) enum SignaturePredicateSubject {
    This,
    Parameter(std::sync::Arc<str>),
}

#[derive(Clone)]
pub(super) struct SignatureParameterRecipe {
    pub(super) declaration: DeclarationIdentity,
    pub(super) location: Location,
    pub(super) annotation: Option<TypeExpression>,
    pub(super) initializer: Option<Expression>,
    pub(super) question_mark: bool,
    pub(super) rest: bool,
    pub(super) function_kind: SyntaxKind,
    pub(super) object_literal_method: bool,
}

#[derive(Clone)]
pub(super) enum SignatureReturnRecipe {
    Annotation(Box<TypeExpression>),
    Constructor {
        class: DeclarationIdentity,
    },
    Bodyless,
    Infer,
    GetterAccessor {
        declaration: DeclarationIdentity,
        body: bool,
    },
}

/// Immutable syntax recipe for `checkExpression`, finalized before sharing.
#[derive(Clone)]
pub(super) struct Expression(std::sync::Arc<ExpressionData>);

impl std::ops::Deref for Expression {
    type Target = ExpressionData;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Expression {
    fn new(data: ExpressionData) -> Self {
        Self(std::sync::Arc::new(data))
    }
}

pub(super) struct ExpressionData {
    pub(super) location: Location,
    // The actual expression, before parenthesis removal, owns checker caches.
    // A failure recipe has no usable expression; its original cause is retained.
    pub(super) root: Option<ExpressionRoot>,
    pub(super) form: ExpressionForm,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct ExpressionRoot {
    pub(super) file: FileId,
    pub(super) site: super::super::binding::flow::Site,
}
// Source attachment identity is exactly two fixed IDs; Eq/Hash observes no
// source text or owned payload, including on node-cache hits.

/// An actual captured expression or native `createSyntheticExpression` node.
/// A synthetic node keeps a fresh identity and its real parent; the parent's
/// kind/identity never stands in for the synthetic node's own syntax. Its
/// diagnostic range is supplied independently by the producing operation.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ExpressionNode {
    Captured(ExpressionRoot),
    Synthetic { id: usize, parent: ExpressionRoot },
}

impl From<ExpressionRoot> for ExpressionNode {
    fn from(root: ExpressionRoot) -> Self {
        Self::Captured(root)
    }
}

impl ExpressionNode {
    /// Native GetSourceFileOfNode follows a synthetic node's actual Parent.
    pub(super) fn file(self) -> FileId {
        match self {
            Self::Captured(root) => root.file,
            Self::Synthetic { parent, .. } => parent.file,
        }
    }
}

pub(super) enum ExpressionForm {
    This(ExpressionRoot),
    Super(ExpressionRoot),
    Template(ExpressionRoot),
    Instantiation(ExpressionRoot),
    Spread(ExpressionRoot),
    Await(Box<Expression>),
    TypeOf(ExpressionRoot),
    Binary(ExpressionRoot),
    ConditionalValue(ExpressionRoot),
    Delete(ExpressionRoot),
    Function(ExpressionRoot),
    Class(ExpressionRoot),
    Array(ExpressionRoot),
    Assertion(ExpressionRoot),
    NonNullAssertion(ExpressionRoot),
    Satisfies(ExpressionRoot),
    Omitted,
    Call(CallRoot),
    Access(ExpressionRoot),
    Identifier {
        file: FileId,
        anchor: Span,
    },
    PrivateIdentifier(ExpressionRoot),
    RegularExpression,
    Null,
    Void,
    Literal {
        base: PrimitiveName,
        value: LiteralValue,
    },
    Unary {
        operator: SyntaxKind,
        direct_literal: Option<PrimitiveName>,
        operand: Box<Expression>,
    },
    Object(ExpressionRoot),
    Pending {
        reason: SemanticFailureReason,
        message: String,
    },
}

/// Provisional reprojection keeps only the captured SourceText between
/// requests. Within one operation each demanded file has one final AST and one
/// parent table shared by selection and all source acquisition operations.
pub(super) struct Source<'p, 'w> {
    inputs: CheckerInputs<'p>,
    parser: &'w Parser,
    control: Option<crate::resources::Control>,
    // Immutable binder associations; all current checker state belongs to Evaluator.
    bindings: super::super::binding::NativeInputs<'p>,
    /// Looked up at every syntax question, never iterated.
    syntax: crate::hash::FastMap<FileId, CapturedSyntax<'p>>,
    array_spreads: HashMap<ExpressionRoot, arrays::SpreadIndices>,
    jsdoc_tags: HashMap<
        DeclarationIdentity,
        Result<std::sync::Arc<crate::parser::jsdoc::Tags>, super::model::DemandError>,
    >,
}

struct CapturedSyntax<'s> {
    ast: Ast<'s>,
    control: Option<crate::resources::Control>,
    parents: Vec<Option<NodeId>>,
    /// Only looked up.
    attachments: crate::hash::FastMap<AttachmentKey, Attachment>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum AttachmentRole {
    Node,
    NativeNode,
    Name,
    NativeDeclaration,
    FirstToken,
    NamelessDeclaration,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct AttachmentKey {
    role: AttachmentRole,
    kind: SyntaxKind,
    start: usize,
    end: usize,
}

enum Attachment {
    Unique(NodeId),
    Ambiguous,
}

impl<'s> CapturedSyntax<'s> {
    // Recipe construction uses this capture's request capability. Ordinary
    // loading has no Control; nested projections share the request recursion guard.
    fn resource_depth(&self) -> Option<crate::resources::DepthGuard> {
        self.control
            .as_ref()
            .map(|control| crate::resources::complete_or_abort(control.enter()))
    }

    /// Retain exact `parseLiteralLikeNode` payloads. Scanner normalization is
    /// already complete; projection only copies bytes into request-owned values.
    fn literal_bytes(&self, bytes: &[u8]) -> std::sync::Arc<[u8]> {
        let copied = bytes.to_vec();
        std::sync::Arc::from(copied)
    }

    fn literal_value(&self, bytes: &[u8]) -> LiteralValue {
        LiteralValue::from_bytes(self.literal_bytes(bytes))
    }

    /// The existing declaration anchor (`getFirstToken`/token-span owner),
    /// using this capture's request capability. Loading has no controller.
    /// This changes no name, trivia or reserved-role selection rule.
    fn declaration_anchor(&self, id: NodeId) -> Span {
        match self.control.as_ref() {
            Some(control) => crate::resources::complete_or_abort(
                crate::project::binding::declaration_anchor_controlled(&self.ast, id, control),
            ),
            None => crate::project::binding::declaration_anchor(&self.ast, id),
        }
    }

    /// Capture the final reachable tree once. These are source attachments,
    /// never another token/binding inventory or semantic identities. Parents,
    /// exact node sites, named declarations and reserved-signature anchors
    /// all describe the same accepted final nodes.
    fn new(
        ast: Ast<'s>,
        control: Option<&crate::resources::Control>,
    ) -> Result<Self, crate::resources::Stop> {
        let mut parents = Vec::with_capacity(ast.nodes().len());
        for _ in ast.nodes() {
            parents.push(None);
        }
        // The final tree: the arena also holds nodes a reparse replaced.
        let order: Vec<NodeId> = ast.walk().collect();
        // Three attachments per node, and a name or anchor for some.
        let mut attachments =
            crate::hash::FastMap::with_capacity_and_hasher(order.len() / 2 * 7, Default::default());
        let mut insert = |role, kind, span: Span, id| -> Result<(), crate::resources::Stop> {
            let key = AttachmentKey {
                role,
                kind,
                start: span.start,
                end: span.end,
            };
            attachments
                .entry(key)
                .and_modify(|entry| {
                    if !matches!(entry, Attachment::Unique(previous) if *previous == id) {
                        *entry = Attachment::Ambiguous;
                    }
                })
                .or_insert(Attachment::Unique(id));
            Ok(())
        };
        let token_span = |id| match control {
            Some(control) => ast.token_span_controlled(id, control),
            None => Ok(ast.token_span(id)),
        };
        let first_token_span = |id| match control {
            Some(control) => ast.first_token_span_controlled(id, control),
            None => Ok(ast.first_token_span(id)),
        };
        for id in order {
            let node = ast.node(id);
            insert(AttachmentRole::NativeNode, node.syntax_kind, node.span, id)?;
            insert(AttachmentRole::Node, node.syntax_kind, token_span(id)?, id)?;
            insert(
                AttachmentRole::NativeDeclaration,
                node.syntax_kind,
                match control {
                    Some(control) => {
                        crate::project::binding::declaration_anchor_controlled(&ast, id, control)?
                    }
                    None => crate::project::binding::declaration_anchor(&ast, id),
                },
                id,
            )?;
            if let Some(name) = syntax::id(node, "name") {
                insert(
                    AttachmentRole::Name,
                    node.syntax_kind,
                    token_span(name)?,
                    id,
                )?;
            } else if matches!(
                node.syntax_kind,
                SyntaxKind::FunctionDeclaration | SyntaxKind::ClassDeclaration
            ) {
                // Unnamed declarations may retain a modifier-token anchor
                // (notably an anonymous default export) rather than a name.
                // Capture both actual roles without inventing a name node.
                insert(
                    AttachmentRole::NamelessDeclaration,
                    node.syntax_kind,
                    first_token_span(id)?,
                    id,
                )?;
                for &modifier in syntax::ids(node, "modifiers") {
                    insert(
                        AttachmentRole::NamelessDeclaration,
                        node.syntax_kind,
                        token_span(modifier)?,
                        id,
                    )?;
                }
            }
            // The retained binder's unnamed reserved-signature associations
            // use getFirstToken, a distinct anchor from a whole-node site.
            if matches!(
                node.syntax_kind,
                SyntaxKind::CallSignature
                    | SyntaxKind::ConstructSignature
                    | SyntaxKind::IndexSignature
                    | SyntaxKind::FunctionType
                    | SyntaxKind::ConstructorType
            ) {
                insert(
                    AttachmentRole::FirstToken,
                    node.syntax_kind,
                    first_token_span(id)?,
                    id,
                )?;
            }
            for child in ast.children(id) {
                parents[child.index()] = Some(id);
            }
        }
        Ok(CapturedSyntax {
            ast,
            control: control.cloned(),
            parents,
            attachments,
        })
    }

    fn attachment(
        &self,
        role: AttachmentRole,
        kind: SyntaxKind,
        span: Span,
    ) -> Result<Option<NodeId>, AnalysisError> {
        let key = AttachmentKey {
            role,
            kind,
            start: span.start,
            end: span.end,
        };
        match self.attachments.get(&key) {
            None => Ok(None),
            Some(Attachment::Unique(id)) => Ok(Some(*id)),
            Some(Attachment::Ambiguous) => Err(super::internal(
                "retained site has several final syntax/role attachments",
            )),
        }
    }

    /// Attach one retained final-tree site by exact token range and syntax
    /// role. Distinct nodes may share a range; ambiguous same-role attachment
    /// is a failure rather than an arbitrary parser arena choice.
    fn node_at(&self, site: crate::project::binding::flow::Site) -> Result<NodeId, AnalysisError> {
        self.attachment(AttachmentRole::Node, site.kind, site.span)?
            .ok_or_else(|| super::internal("retained site has no final syntax/role attachment"))
    }

    /// Attach the actual own declaration family selected by the binder's
    /// native type/value question. Related declaration kinds do not become
    /// interchangeable merely because their names share a span.
    fn declaration_at(
        &self,
        kinds: &[SyntaxKind],
        anchor: Span,
    ) -> Result<Option<NodeId>, AnalysisError> {
        let mut found = None;
        for &kind in kinds {
            if let Some(id) = self.attachment(AttachmentRole::Name, kind, anchor)? {
                if found.is_some_and(|previous| previous != id) {
                    return Err(super::internal(
                        "canonical declaration has several final syntax/role attachments",
                    ));
                }
                found = Some(id);
            }
        }
        Ok(found)
    }

    /// Attach a retained Symbol.Declarations entry using the binder's own
    /// declaration-anchor rule, including anonymous and reserved symbols.
    fn native_declaration_at(
        &self,
        kind: SyntaxKind,
        anchor: Span,
    ) -> Result<Option<NodeId>, AnalysisError> {
        self.attachment(AttachmentRole::NativeDeclaration, kind, anchor)
    }

    /// Join an actual native node fact by its complete span and syntax role;
    /// declaration anchors and token spans answer different questions.
    fn native_node_at(&self, kind: SyntaxKind, span: Span) -> Result<NodeId, AnalysisError> {
        self.attachment(AttachmentRole::NativeNode, kind, span)?
            .ok_or_else(|| super::internal("native node lacks its final syntax attachment"))
    }

    /// `isNodeDescendantOf` includes the node itself and follows actual parents.
    fn node_is_descendant(&self, node: NodeId, ancestor: NodeId) -> bool {
        let mut current = Some(node);
        while let Some(node) = current {
            if node == ancestor {
                return true;
            }
            current = self.parents[node.index()];
        }
        false
    }
}

impl<'p, 'w> Source<'p, 'w> {
    pub(super) fn bindings(&self) -> super::super::binding::NativeInputs<'p> {
        self.bindings
    }
    pub(super) fn new(inputs: CheckerInputs<'p>, parser: &'w Parser) -> Self {
        let control = inputs.control.cloned();
        Self {
            inputs,
            parser,
            control,
            bindings: crate::project::binding::NativeInputs::new(
                inputs.bindings,
                inputs.raw_bindings,
            ),
            syntax: crate::hash::FastMap::default(),
            jsdoc_tags: HashMap::new(),
            array_spreads: HashMap::new(),
        }
    }

    fn syntax(&mut self, file: FileId) -> Result<&CapturedSyntax<'p>, AnalysisError> {
        let entry = match self.syntax.entry(file) {
            std::collections::hash_map::Entry::Occupied(entry) => return Ok(entry.into_mut()),
            std::collections::hash_map::Entry::Vacant(entry) => entry,
        };
        let input = self.inputs.sources.get(file);
        // Loading already accepted this exact path as an identity.
        let name = input
            .path
            .to_str()
            .expect("captured input path is exact Unicode");
        let ast = self
            .parser
            .parse_captured(input.text.as_str(), name, self.control.clone())
            .map_err(|error| match error {
                crate::parser::Failure::Syntax(error) => {
                    InputError::unparsable(error, &input.text, &input.path).into()
                }
                crate::parser::Failure::Interrupted(reason) => super::resource_failure(reason),
            })?;
        let captured =
            CapturedSyntax::new(ast, self.control.as_ref()).map_err(super::resource_failure)?;
        Ok(entry.insert(captured))
    }

    /// The request's `getResolvedSignature` prelude visits the selected site's
    /// actual containing call/new/tag nodes, outermost first. This records
    /// syntax dependencies only; acquiring their signatures belongs to the
    /// evaluator, and candidate collection must not perform checking.
    pub(super) fn enclosing_calls(
        &mut self,
        file: FileId,
        offset: usize,
    ) -> Result<Vec<CallRoot>, AnalysisError> {
        let CapturedSyntax { ast, parents, .. } = self.syntax(file)?;
        let Some(mut site) = ast
            .walk()
            .filter(|&id| {
                let span = ast.token_span(id);
                span.start <= offset && offset < span.end
            })
            .min_by_key(|&id| {
                let span = ast.token_span(id);
                span.end - span.start
            })
        else {
            return Ok(Vec::new());
        };
        let mut calls = Vec::new();
        loop {
            if matches!(
                ast.node(site).syntax_kind,
                SyntaxKind::CallExpression
                    | SyntaxKind::NewExpression
                    | SyntaxKind::TaggedTemplateExpression
            ) {
                calls.push(CallRoot::new(
                    file,
                    ast.token_span(site),
                    ast.node(site).syntax_kind,
                ));
            }
            let Some(parent) = parents[site.index()] else {
                break;
            };
            site = parent;
        }
        calls.reverse();
        Ok(calls)
    }

    /// Whether `file`'s syntax is captured for this request.
    pub(super) fn holds_syntax(&self, file: FileId) -> bool {
        self.syntax.contains_key(&file)
    }

    /// Release `file`'s captured syntax. Its handles stay valid: capturing it
    /// again parses the same text into the same nodes, spans and attachments.
    pub(super) fn release_syntax(&mut self, file: FileId) {
        self.syntax.remove(&file);
    }

    /// Every object literal of a file, in source order (Q5's examined
    /// literals).
    pub(super) fn object_literals(
        &mut self,
        file: FileId,
    ) -> Result<Vec<ExpressionRoot>, AnalysisError> {
        let CapturedSyntax { ast, .. } = self.syntax(file)?;
        let mut literals: Vec<ExpressionRoot> = ast
            .walk()
            .filter(|&id| ast.node(id).syntax_kind == SyntaxKind::ObjectLiteralExpression)
            .map(|id| ExpressionRoot {
                file,
                site: crate::project::binding::flow::Site::new(ast, id),
            })
            .collect();
        literals.sort_by_key(|root| (root.site.span.start, root.site.span.end));
        Ok(literals)
    }

    /// `getSymbolAtLocation` / `getTypeOfAlias`: a named import or export's
    /// two name tokens address its local alias symbol. Other semantic token
    /// families remain explicit pending operations at this implementation
    /// checkpoint; trivia is never promoted to a nearest declaration.
    pub(super) fn select(
        &mut self,
        file: FileId,
        offset: usize,
    ) -> Result<Option<SelectedSyntax>, AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(file)?;
        let CapturedSyntax { ast, parents, .. } = captured;
        for id in ast.walk() {
            let token = ast.token_span(id);
            if !(token.start <= offset && offset < token.end) {
                continue;
            }
            let kind = ast.node(id).syntax_kind;
            // Q4's useful expression anchors select the actual access/call
            // operation. A key literal still selects itself. These terminal
            // positions come from accepted complete grammar nodes, whose end
            // excludes trailing trivia; no nearby-token search is involved.
            let terminal = token.end.checked_sub(1) == Some(offset);
            let operation_anchor = match kind {
                SyntaxKind::PropertyAccessExpression => syntax::id(ast.node(id), "name")
                    .is_some_and(|name| {
                        let name = ast.token_span(name);
                        name.start <= offset && offset < name.end
                    }),
                SyntaxKind::ElementAccessExpression
                | SyntaxKind::CallExpression
                | SyntaxKind::TaggedTemplateExpression => terminal,
                SyntaxKind::NewExpression => {
                    let first = ast.first_token_span(id);
                    first.start <= offset && offset < first.end
                        || terminal && ast.node(id).field("arguments").is_some()
                }
                _ => false,
            };
            if operation_anchor {
                return Ok(Some(SelectedSyntax::Expression(expression(
                    inputs, file, captured, id,
                ))));
            }
            if let Some(parent) = parents[id.index()] {
                let member = ast.node(parent);
                // Q4 resolves declaration aliases before interpreting a quoted
                // source/export name as a string expression. Both renamed sides
                // carry the same actual alias declaration and their own token.
                if matches!(
                    member.syntax_kind,
                    SyntaxKind::ImportSpecifier
                        | SyntaxKind::ExportSpecifier
                        | SyntaxKind::ImportClause
                        | SyntaxKind::NamespaceImport
                        | SyntaxKind::NamespaceExport
                        | SyntaxKind::ImportEqualsDeclaration
                ) && (syntax::id(member, "name") == Some(id)
                    || matches!(
                        member.syntax_kind,
                        SyntaxKind::ImportSpecifier | SyntaxKind::ExportSpecifier
                    ) && syntax::id(member, "propertyName") == Some(id))
                {
                    let type_only = inputs
                        .bindings
                        .alias_syntax_is_type_only(file, member.syntax_kind, member.span)
                        .map_err(|error| {
                            AnalysisError::from(super::binding_failure_at(
                                inputs,
                                &super::location(inputs, file, token),
                                error,
                            ))
                        })?;
                    return Ok(Some(SelectedSyntax::Alias {
                        declaration: DeclarationIdentity::new(file, parent),
                        type_only,
                        token,
                    }));
                }
                if member.syntax_kind == SyntaxKind::EnumMember
                    && syntax::id(member, "name") == Some(id)
                {
                    return Ok(Some(SelectedSyntax::Declaration {
                        anchor: token,
                        kind: DeclarationKind::EnumMember,
                        declaration: DeclarationIdentity::new(file, parent),
                    }));
                }
                if matches!(
                    kind,
                    SyntaxKind::Identifier
                        | SyntaxKind::PrivateIdentifier
                        | SyntaxKind::StringLiteral
                        | SyntaxKind::NumericLiteral
                ) && syntax::id(member, "name") == Some(id)
                    && matches!(
                        member.syntax_kind,
                        SyntaxKind::PropertySignature
                            | SyntaxKind::PropertyDeclaration
                            | SyntaxKind::MethodSignature
                            | SyntaxKind::MethodDeclaration
                            | SyntaxKind::GetAccessor
                            | SyntaxKind::SetAccessor
                    )
                    && parents[parent.index()].is_some_and(|container| {
                        matches!(
                            ast.node(container).syntax_kind,
                            SyntaxKind::InterfaceDeclaration
                                | SyntaxKind::ClassDeclaration
                                | SyntaxKind::ClassExpression
                                | SyntaxKind::TypeLiteral
                                | SyntaxKind::ObjectLiteralExpression
                        )
                    })
                {
                    return Ok(Some(SelectedSyntax::Member {
                        anchor: token,
                        declaration: DeclarationIdentity::new(file, parent),
                    }));
                }
            }
            // getTypeOfNode treats `this` as an expression independently of
            // identifier spelling or declaration-name selection. Its checker
            // owns contextual and control-flow acquisition at this exact node.
            if matches!(kind, SyntaxKind::ThisKeyword | SyntaxKind::SuperKeyword) {
                return Ok(Some(SelectedSyntax::Expression(expression(
                    inputs, file, captured, id,
                ))));
            }
            if kind == SyntaxKind::TypeQuery {
                let keyword = ast.first_token_span(id);
                if keyword.start <= offset && offset < keyword.end {
                    return Ok(Some(SelectedSyntax::Type(type_expression(
                        inputs, file, captured, parents, id,
                    ))));
                }
            }
            if kind == SyntaxKind::ThisType
                || intrinsic(kind).is_some()
                || special(kind).is_some() && kind != SyntaxKind::NullKeyword
                || kind == SyntaxKind::NullKeyword
                    && parents[id.index()].is_some_and(|parent| {
                        ast.node(parent).syntax_kind == SyntaxKind::LiteralType
                    })
            {
                return Ok(Some(SelectedSyntax::Type(type_expression(
                    inputs, file, captured, parents, id,
                ))));
            }
            if matches!(
                kind,
                SyntaxKind::StringLiteral
                    | SyntaxKind::NumericLiteral
                    | SyntaxKind::BigIntLiteral
                    | SyntaxKind::TrueKeyword
                    | SyntaxKind::FalseKeyword
                    | SyntaxKind::NoSubstitutionTemplateLiteral
                    | SyntaxKind::NullKeyword
            ) {
                if let Some(parent) = parents[id.index()]
                    .filter(|&parent| ast.node(parent).syntax_kind == SyntaxKind::LiteralType)
                {
                    return Ok(Some(SelectedSyntax::Type(type_expression(
                        inputs, file, captured, parents, parent,
                    ))));
                }
                if syntax::is_expression_node(ast, parents, id) {
                    return Ok(Some(SelectedSyntax::Expression(expression(
                        inputs, file, captured, id,
                    ))));
                }
                if let Some(selected) = property_value_selection(file, ast, parents, id)? {
                    return Ok(Some(selected));
                }
                // Q4 deliberately has no module-specifier type anchor.
                // Its captured text remains a module identity, not a cooked
                // string expression with a fabricated successful type.
                if parents[id.index()].is_some_and(|parent| {
                    let parent = ast.node(parent);
                    matches!(
                        parent.syntax_kind,
                        SyntaxKind::ImportDeclaration | SyntaxKind::ExportDeclaration
                    ) && syntax::id(parent, "moduleSpecifier") == Some(id)
                        || parent.syntax_kind == SyntaxKind::ExternalModuleReference
                            && syntax::id(parent, "expression") == Some(id)
                }) {
                    return Ok(Some(SelectedSyntax::UnsupportedTarget { token }));
                }
                return Ok(Some(SelectedSyntax::Pending { token }));
            }
            if kind != SyntaxKind::Identifier {
                continue;
            }
            if let Some(parent) = parents[id.index()] {
                let node = ast.node(parent);
                if syntax::id(node, "name") == Some(id) {
                    if let Some(selected) = property_value_selection(file, ast, parents, id)? {
                        return Ok(Some(selected));
                    }
                    let kind = match node.syntax_kind {
                        SyntaxKind::VariableDeclaration
                        | SyntaxKind::Parameter
                        | SyntaxKind::BindingElement => Some(DeclarationKind::Variable),
                        SyntaxKind::FunctionDeclaration => Some(DeclarationKind::Function),
                        SyntaxKind::TypeAliasDeclaration => Some(DeclarationKind::TypeAlias),
                        SyntaxKind::ClassDeclaration | SyntaxKind::ClassExpression => {
                            Some(DeclarationKind::Class)
                        }
                        SyntaxKind::InterfaceDeclaration => Some(DeclarationKind::Interface),
                        SyntaxKind::TypeParameter => Some(DeclarationKind::TypeParameter),
                        SyntaxKind::EnumDeclaration => Some(DeclarationKind::Enum),
                        SyntaxKind::EnumMember => Some(DeclarationKind::EnumMember),
                        _ => None,
                    };
                    if let Some(kind) = kind {
                        return Ok(Some(SelectedSyntax::Declaration {
                            anchor: token,
                            kind,
                            declaration: DeclarationIdentity::new(file, parent),
                        }));
                    }
                }
                if node.syntax_kind == SyntaxKind::TypeReference
                    && syntax::id(node, "typeName") == Some(id)
                {
                    return Ok(Some(SelectedSyntax::Type(type_expression(
                        inputs, file, captured, parents, parent,
                    ))));
                }
            }
            // A one-name import-type qualifier is the node's final name and
            // selects its type, as a type reference's name does. A name in a
            // qualified chain is a namespace path segment, which no anchor
            // types (the final name's parent is a QualifiedName, not a type).
            if let Some(import) = syntax::import_type_of_qualifier_part(ast, parents, id) {
                return Ok(Some(if parents[id.index()] == Some(import) {
                    SelectedSyntax::Type(type_expression(inputs, file, captured, parents, import))
                } else {
                    SelectedSyntax::UnsupportedTarget { token }
                }));
            }
            if syntax::is_in_type_query(ast, |id| parents[id.index()], id) {
                return Ok(Some(SelectedSyntax::ValueAt(type_query_operand(
                    inputs, file, captured, parents, id,
                ))));
            }
            if syntax::is_expression_node(ast, parents, id) {
                return Ok(Some(SelectedSyntax::Expression(expression(
                    inputs, file, captured, id,
                ))));
            }
            return Ok(Some(SelectedSyntax::Pending { token }));
        }
        Ok(None)
    }

    pub(super) fn type_alias(
        &mut self,
        declaration: BoundDeclaration,
    ) -> Result<TypeExpression, AnalysisError> {
        let inputs = self.inputs;
        let captured = self.syntax(declaration.file)?;
        let id = captured
            .declaration_at(&[SyntaxKind::TypeAliasDeclaration], declaration.anchor)?
            .ok_or_else(|| {
                super::internal("canonical type alias has no final declaration/name attachment")
            })?;
        let CapturedSyntax { ast, parents, .. } = captured;
        let node = ast.node(id);
        let body = syntax::id(node, "type")
            .ok_or_else(|| super::internal("parsed type alias has no body"))?;
        Ok(type_expression(
            inputs,
            declaration.file,
            captured,
            parents,
            body,
        ))
    }
}

/// Syntax recipes for `getTypeFromTypeNodeWorker`. Construction/cache state is
/// evaluation's concern; unknown forms are never converted to an intrinsic.
fn type_expression(
    inputs: CheckerInputs<'_>,
    file: FileId,
    captured: &CapturedSyntax<'_>,
    parents: &[Option<NodeId>],
    id: NodeId,
) -> TypeExpression {
    let _depth = captured.resource_depth();
    let ast = &captured.ast;
    let node = ast.node(id);
    let form = if let Some(primitive) = intrinsic(node.syntax_kind) {
        TypeExpressionForm::Primitive(primitive)
    } else if let Some(name) = special(node.syntax_kind) {
        TypeExpressionForm::Special(name)
    } else if node.syntax_kind == SyntaxKind::IntrinsicKeyword {
        TypeExpressionForm::IntrinsicMarker
    } else if node.syntax_kind == SyntaxKind::MappedType {
        TypeExpressionForm::Mapped(ExpressionRoot {
            file,
            site: super::super::binding::flow::Site::new(ast, id),
        })
    } else if node.syntax_kind == SyntaxKind::TemplateLiteralType {
        TypeExpressionForm::TemplateLiteral(ExpressionRoot {
            file,
            site: super::super::binding::flow::Site::new(ast, id),
        })
    } else if node.syntax_kind == SyntaxKind::ThisType {
        match generics::this_type_context(ast, parents, id) {
            Ok(context) => TypeExpressionForm::This(context),
            Err(message) => type_pending(message),
        }
    } else if node.syntax_kind == SyntaxKind::TypePredicate {
        // `getTypeFromTypeNodeWorker` returns only boolean/void for this
        // question. The asserted type belongs to getTypePredicateOfSignature.
        if syntax::id(node, "assertsModifier").is_some() {
            TypeExpressionForm::Special(SpecialName::Void)
        } else {
            TypeExpressionForm::Primitive(PrimitiveName::Boolean)
        }
    } else if node.syntax_kind == SyntaxKind::LiteralType
        && let Some(literal) = syntax::id(node, "literal")
    {
        // `getTypeFromLiteralTypeNode` returns nullType directly; other
        // literals consume the regular form of checkExpression.
        if ast.node(literal).syntax_kind == SyntaxKind::NullKeyword {
            TypeExpressionForm::Special(SpecialName::Null)
        } else {
            TypeExpressionForm::Literal(expression(inputs, file, captured, literal))
        }
    } else if matches!(
        node.syntax_kind,
        SyntaxKind::ArrayType | SyntaxKind::TupleType
    ) {
        TypeExpressionForm::ArrayOrTuple(tuples::array_or_tuple_recipe(
            inputs, file, captured, parents, id,
        ))
    } else if matches!(
        node.syntax_kind,
        SyntaxKind::NamedTupleMember | SyntaxKind::RestType | SyntaxKind::OptionalType
    ) {
        TypeExpressionForm::TupleElement(tuples::element_recipe(
            inputs, file, captured, parents, id,
        ))
    } else if node.syntax_kind == SyntaxKind::TypeOperator
        && syntax::number(node, "operator") == Some(SyntaxKind::KeyOfKeyword.number())
        && let Some(value) = syntax::id(node, "type")
    {
        TypeExpressionForm::Keyof {
            site: super::super::binding::flow::Site::new(ast, id),
            value: Box::new(type_expression(inputs, file, captured, parents, value)),
        }
    } else if node.syntax_kind == SyntaxKind::TypeOperator
        && syntax::number(node, "operator") == Some(SyntaxKind::UniqueKeyword.number())
    {
        TypeExpressionForm::Unique(ExpressionRoot {
            file,
            site: super::super::binding::flow::Site::new(ast, id),
        })
    } else if tuples::readonly_operator(ast, id)
        && let Some(value) = syntax::id(node, "type")
    {
        TypeExpressionForm::Readonly {
            site: super::super::binding::flow::Site::new(ast, id),
            value: Box::new(type_expression(inputs, file, captured, parents, value)),
        }
    } else if matches!(
        node.syntax_kind,
        SyntaxKind::TypeLiteral | SyntaxKind::FunctionType | SyntaxKind::ConstructorType
    ) {
        TypeExpressionForm::TypeLiteral {
            anchor: ast.first_token_span(id),
            alias: alias_for_type_node(file, captured, parents, id),
        }
    } else if node.syntax_kind == SyntaxKind::ConditionalType {
        TypeExpressionForm::Conditional(ExpressionRoot {
            file,
            site: super::super::binding::flow::Site::new(ast, id),
        })
    } else if node.syntax_kind == SyntaxKind::InferType {
        TypeExpressionForm::Infer(ExpressionRoot {
            file,
            site: super::super::binding::flow::Site::new(ast, id),
        })
    } else if node.syntax_kind == SyntaxKind::IndexedAccessType
        && let (Some(object), Some(index)) = (
            syntax::id(node, "objectType"),
            syntax::id(node, "indexType"),
        )
    {
        TypeExpressionForm::IndexedAccess {
            site: super::super::binding::flow::Site::new(ast, id),
            object: Box::new(type_expression(inputs, file, captured, parents, object)),
            index: Box::new(type_expression(inputs, file, captured, parents, index)),
            alias: alias_for_type_node(file, captured, parents, id),
        }
    } else if matches!(
        node.syntax_kind,
        SyntaxKind::UnionType | SyntaxKind::IntersectionType
    ) {
        let site = super::super::binding::flow::Site::new(ast, id);
        let types = syntax::ids(node, "types")
            .iter()
            .map(|&id| type_expression(inputs, file, captured, parents, id))
            .collect();
        let alias = alias_for_type_node(file, captured, parents, id);
        if node.syntax_kind == SyntaxKind::UnionType {
            TypeExpressionForm::Union { site, types, alias }
        } else {
            TypeExpressionForm::Intersection { site, types, alias }
        }
    } else if node.syntax_kind == SyntaxKind::ParenthesizedType
        && let Some(inner) = syntax::id(node, "type")
    {
        // `getTypeFromTypeNodeWorker` delegates the complete operation to
        // the child, including the location of any consumed failure.
        // getAliasSymbolForTypeNode separately preserves alias parentage;
        // selection retains its own root independently of this recipe.
        return type_expression(inputs, file, captured, parents, inner);
    } else if node.syntax_kind == SyntaxKind::TypeQuery {
        TypeExpressionForm::Query(ExpressionRoot {
            file,
            site: super::super::binding::flow::Site::new(ast, id),
        })
    } else if node.syntax_kind == SyntaxKind::ImportType {
        TypeExpressionForm::Import(ExpressionRoot {
            file,
            site: super::super::binding::flow::Site::new(ast, id),
        })
    } else if matches!(
        node.syntax_kind,
        SyntaxKind::TypeReference | SyntaxKind::ExpressionWithTypeArguments
    ) {
        TypeExpressionForm::Reference {
            site: super::super::binding::flow::Site::new(ast, id),
            anchor: unresolved::type_reference_name_node(ast, id)
                .and_then(|name| entity_name_anchor(ast, name)),
            arguments: syntax::ids(node, "typeArguments")
                .iter()
                .map(|&argument| type_expression(inputs, file, captured, parents, argument))
                .collect(),
            alias: alias_for_type_node(file, captured, parents, id),
            deferred: (node.syntax_kind == SyntaxKind::TypeReference).then(|| {
                tuples::deferred_reference_recipe(
                    inputs,
                    file,
                    captured,
                    parents,
                    id,
                    syntax::ids(node, "typeArguments"),
                )
            }),
        }
    } else {
        type_pending("native type-node operation is pending")
    };
    TypeExpression::new(TypeExpressionData {
        file,
        root: Some(ExpressionRoot {
            file,
            site: super::super::binding::flow::Site::new(ast, id),
        }),
        location: super::location(inputs, file, ast.token_span(id)),
        form,
    })
}

/// Retain the terminal binder attachment of an actual entity-name chain.
/// The calling syntax operation decides which native name kinds it accepts;
/// getTypeReferenceName's complete selection lives in source::unresolved.
fn entity_name_anchor(ast: &Ast<'_>, mut id: NodeId) -> Option<Span> {
    let mut terminal = None;
    loop {
        let node = ast.node(id);
        match node.syntax_kind {
            SyntaxKind::Identifier => return Some(terminal.unwrap_or_else(|| ast.token_span(id))),
            SyntaxKind::QualifiedName | SyntaxKind::PropertyAccessExpression => {
                let qualified = node.syntax_kind == SyntaxKind::QualifiedName;
                let right = syntax::id(node, if qualified { "right" } else { "name" })?;
                if ast.node(right).syntax_kind != SyntaxKind::Identifier {
                    return None;
                }
                terminal.get_or_insert_with(|| ast.token_span(right));
                id = syntax::id(node, if qualified { "left" } else { "expression" })?;
            }
            _ => return None,
        }
    }
}

fn type_pending(message: impl Into<String>) -> TypeExpressionForm {
    TypeExpressionForm::Pending {
        reason: SemanticFailureReason::UnsupportedSemanticForm,
        message: message.into(),
    }
}

/// Capture `checkExpressionWithTypeArguments`'s TypeQuery expression name.
/// `getRegularTypeOfExpression` promotes only the right side to its immediate
/// qualified operation. Earlier segments retain their own receiver question.
/// A `this` operand has its separate checkThisExpression operation.
fn type_query_operand(
    inputs: CheckerInputs<'_>,
    file: FileId,
    captured: &CapturedSyntax<'_>,
    parents: &[Option<NodeId>],
    id: NodeId,
) -> Expression {
    let ast = &captured.ast;
    let id = parents[id.index()]
        .filter(|&parent| {
            ast.node(parent).syntax_kind == SyntaxKind::QualifiedName
                && syntax::id(ast.node(parent), "right") == Some(id)
        })
        .unwrap_or(id);
    if syntax::is_this_identifier(ast, id) {
        let root = ExpressionRoot {
            file,
            site: super::super::binding::flow::Site::new(ast, id),
        };
        Expression::new(ExpressionData {
            location: super::location(inputs, file, ast.token_span(id)),
            root: Some(root),
            form: ExpressionForm::This(root),
        })
    } else {
        expression(inputs, file, captured, id)
    }
}

/// Source inputs for `getSignatureFromDeclaration`. Parameter read types,
/// optionality and return checking are evaluator questions, not syntax guesses.
fn signature_recipe(
    inputs: CheckerInputs<'_>,
    file: FileId,
    captured: &CapturedSyntax<'_>,
    parents: &[Option<NodeId>],
    id: NodeId,
) -> Result<SignatureRecipe, &'static str> {
    let ast = &captured.ast;
    let node = ast.node(id);
    let root = signature_root(ast, file, id)?;
    let role = root.role;
    let type_parameters = syntax::ids(node, "typeParameters")
        .iter()
        .map(|&parameter| {
            syntax::id(ast.node(parameter), "name")
                .filter(|&name| ast.node(name).syntax_kind == SyntaxKind::Identifier)
                .ok_or("native signature type-parameter declaration role is pending")?;
            Ok(DeclarationIdentity::new(file, parameter))
        })
        .collect::<Result<Vec<_>, &'static str>>()?;
    let mut this_parameter = None;
    let mut specialized = false;
    let mut parameters = Vec::new();
    for (index, &id) in syntax::ids(node, "parameters").iter().enumerate() {
        let parameter = ast.node(id);
        specialized |= syntax::id(parameter, "type")
            .is_some_and(|id| ast.node(id).syntax_kind == SyntaxKind::LiteralType);
        let recipe = signature_parameter_recipe(inputs, file, captured, parents, id)?;
        if index == 0 && syntax::is_this_parameter(ast, id) {
            this_parameter = Some(recipe);
        } else {
            parameters.push(recipe);
        }
    }
    // `isOptionalParameter`'s initialized-parameter branch asks the complete
    // getMinArgumentCountEx owner, which demands a final rest type and may
    // derive required tuple elements. The current non-tuple family cannot
    // advertise a syntactic count as that established semantic answer.
    if parameters.iter().any(|parameter| parameter.rest)
        && this_parameter
            .iter()
            .chain(&parameters)
            .any(|parameter| parameter.initializer.is_some())
    {
        return Err(
            "native initialized-parameter rest arity and tuple required-count construction are pending",
        );
    }
    let span = ast.token_span(id);
    let predicate = syntax::id(node, "type")
        .filter(|&annotation| ast.node(annotation).syntax_kind == SyntaxKind::TypePredicate)
        .map(|annotation| signature_predicate_recipe(inputs, file, captured, parents, annotation))
        .transpose()?;
    let return_type = signature_return_recipe(inputs, file, captured, parents, id, role)?;
    let origin = syntax::id(node, "name")
        .map(|name| ast.token_span(name))
        .unwrap_or(span);
    // `getSignatureFromDeclaration` / `hasRestParameter`: only the last
    // actual parameter determines HasRestParameter; a literal annotation
    // records specialization without demanding that annotation's type.
    let flags = SignatureFlags::declared(
        syntax::ids(node, "parameters")
            .last()
            .is_some_and(|&id| syntax::id(ast.node(id), "dotDotDotToken").is_some()),
        specialized,
        matches!(
            role,
            SignatureRole::ConstructorType | SignatureRole::Constructor | SignatureRole::Construct
        ),
        node.syntax_kind == SyntaxKind::ConstructorType
            && syntax::has_modifier(ast, node, SyntaxKind::AbstractKeyword)
            || node.syntax_kind == SyntaxKind::Constructor
                && parents[id.index()].is_some_and(|parent| {
                    syntax::has_modifier(ast, ast.node(parent), SyntaxKind::AbstractKeyword)
                }),
    );
    let untyped = inputs.in_javascript_file(file)
        && matches!(
            role,
            SignatureRole::FunctionExpression
                | SignatureRole::ArrowFunction
                | SignatureRole::MethodDeclaration
                | SignatureRole::Getter
                | SignatureRole::Setter
                | SignatureRole::FunctionDeclaration
                | SignatureRole::Constructor
        )
        && syntax::immediately_invoked_function(ast, parents, id).is_none()
        && syntax::ids(node, "parameters")
            .iter()
            .all(|&parameter| syntax::id(ast.node(parameter), "type").is_none());
    let in_object_literal = parents[id.index()]
        .is_some_and(|parent| ast.node(parent).syntax_kind == SyntaxKind::ObjectLiteralExpression);
    let untyped_javascript = match role {
        _ if !untyped => UntypedJavaScript::No,
        SignatureRole::FunctionExpression | SignatureRole::ArrowFunction => {
            UntypedJavaScript::Expression
        }
        SignatureRole::MethodDeclaration | SignatureRole::Getter | SignatureRole::Setter
            if in_object_literal =>
        {
            UntypedJavaScript::Expression
        }
        _ => UntypedJavaScript::Declaration,
    };
    Ok(SignatureRecipe {
        root,
        type_parameters,
        this_parameter,
        parameters,
        return_type,
        predicate,
        origin: super::location(inputs, file, origin),
        flags,
        untyped_javascript,
    })
}

/// Source inputs for `getReturnTypeFromAnnotation`, independent of parameter
/// acquisition and signature construction.
fn signature_return_recipe(
    inputs: CheckerInputs<'_>,
    file: FileId,
    captured: &CapturedSyntax<'_>,
    parents: &[Option<NodeId>],
    id: NodeId,
    role: SignatureRole,
) -> Result<SignatureReturnRecipe, &'static str> {
    let ast = &captured.ast;
    let node = ast.node(id);
    Ok(if role == SignatureRole::Constructor {
        let class = parents[id.index()]
            .filter(|&parent| {
                matches!(
                    ast.node(parent).syntax_kind,
                    SyntaxKind::ClassDeclaration | SyntaxKind::ClassExpression
                )
            })
            .ok_or("native constructor signature lacks its containing class")?;
        SignatureReturnRecipe::Constructor {
            class: DeclarationIdentity::new(file, class),
        }
    } else if let Some(annotation) = syntax::id(node, "type") {
        SignatureReturnRecipe::Annotation(Box::new(type_expression(
            inputs, file, captured, parents, annotation,
        )))
    } else if role == SignatureRole::Getter {
        syntax::id(node, "name").ok_or("native getter signature has no declaration name")?;
        SignatureReturnRecipe::GetterAccessor {
            declaration: DeclarationIdentity::new(file, id),
            body: syntax::id(node, "body").is_some(),
        }
    } else if syntax::id(node, "body").is_some() {
        SignatureReturnRecipe::Infer
    } else {
        SignatureReturnRecipe::Bodyless
    })
}

/// The actual signature declaration role is metadata independent of its
/// parameters or return type. Body inference can retain this root without
/// demanding getSignatureFromDeclaration's unrelated parameter questions.
fn signature_root(ast: &Ast<'_>, file: FileId, id: NodeId) -> Result<SignatureRoot, &'static str> {
    let role = match ast.node(id).syntax_kind {
        SyntaxKind::FunctionDeclaration => SignatureRole::FunctionDeclaration,
        SyntaxKind::FunctionExpression => SignatureRole::FunctionExpression,
        SyntaxKind::ArrowFunction => SignatureRole::ArrowFunction,
        SyntaxKind::FunctionType => SignatureRole::FunctionType,
        SyntaxKind::ConstructorType => SignatureRole::ConstructorType,
        SyntaxKind::Constructor => SignatureRole::Constructor,
        SyntaxKind::CallSignature => SignatureRole::Call,
        SyntaxKind::ConstructSignature => SignatureRole::Construct,
        SyntaxKind::IndexSignature => SignatureRole::Index,
        SyntaxKind::MethodDeclaration => SignatureRole::MethodDeclaration,
        SyntaxKind::MethodSignature => SignatureRole::MethodSignature,
        SyntaxKind::GetAccessor => SignatureRole::Getter,
        SyntaxKind::SetAccessor => SignatureRole::Setter,
        _ => return Err("native signature syntax role is pending"),
    };
    let span = ast.token_span(id);
    Ok(SignatureRoot {
        file,
        start: span.start,
        end: span.end,
        role,
    })
}

/// Captured inputs for `getTypeForVariableLikeDeclaration`'s Parameter family.
/// Signature display/call questions and ordinary symbol reads share this own
/// syntax operation; annotations retain precedence over inference dependencies.
fn signature_parameter_recipe(
    inputs: CheckerInputs<'_>,
    file: FileId,
    captured: &CapturedSyntax<'_>,
    parents: &[Option<NodeId>],
    id: NodeId,
) -> Result<SignatureParameterRecipe, &'static str> {
    let ast = &captured.ast;
    let parameter = ast.node(id);
    let name = syntax::id(parameter, "name")
        .filter(|&id| {
            matches!(
                ast.node(id).syntax_kind,
                SyntaxKind::Identifier
                    | SyntaxKind::ObjectBindingPattern
                    | SyntaxKind::ArrayBindingPattern
            )
        })
        .ok_or("native signature parameter has no actual binding name")?;
    let function =
        parents[id.index()].ok_or("native parameter declaration lacks a function-like parent")?;
    Ok(SignatureParameterRecipe {
        declaration: DeclarationIdentity::new(file, id),
        location: super::location(inputs, file, ast.token_span(name)),
        annotation: syntax::id(parameter, "type")
            .map(|id| type_expression(inputs, file, captured, parents, id)),
        initializer: syntax::id(parameter, "initializer")
            .map(|id| expression(inputs, file, captured, id)),
        question_mark: syntax::id(parameter, "questionToken").is_some(),
        rest: syntax::id(parameter, "dotDotDotToken").is_some(),
        function_kind: ast.node(function).syntax_kind,
        object_literal_method: syntax::is_object_literal_method(
            ast,
            |node| parents[node.index()],
            function,
        ),
    })
}

/// Captured inputs for `createTypePredicateFromTypePredicateNode`. The evaluator
/// addresses the actual signature symbols; syntax does not recreate generated
/// binding-pattern parameter names. The asserted type remains a lazy question.
fn signature_predicate_recipe(
    inputs: CheckerInputs<'_>,
    file: FileId,
    captured: &CapturedSyntax<'_>,
    parents: &[Option<NodeId>],
    id: NodeId,
) -> Result<SignaturePredicateRecipe, &'static str> {
    let ast = &captured.ast;
    let node = ast.node(id);
    let name = syntax::id(node, "parameterName").ok_or("native predicate has no actual subject")?;
    let subject = if ast.node(name).syntax_kind == SyntaxKind::ThisType {
        SignaturePredicateSubject::This
    } else if let Some(name) = ast.identifier_name(name) {
        SignaturePredicateSubject::Parameter(name.into())
    } else {
        return Err("native predicate subject cannot be represented exactly");
    };
    let value_type =
        syntax::id(node, "type").map(|id| type_expression(inputs, file, captured, parents, id));
    let kind = if syntax::id(node, "assertsModifier").is_some() {
        if value_type.is_some() {
            TypePredicateKind::AssertsIs
        } else {
            TypePredicateKind::Asserts
        }
    } else {
        TypePredicateKind::Is
    };
    Ok(SignaturePredicateRecipe {
        kind,
        subject,
        value_type,
    })
}

/// `getIndexInfosOfIndexSymbol`: malformed explicit shapes are diagnostics,
/// not index entries. Valid entries preserve native value-before-key demand.
fn index_signature_recipe<'src>(
    inputs: CheckerInputs<'_>,
    file: FileId,
    captured: &CapturedSyntax<'src>,
    parents: &[Option<NodeId>],
    id: NodeId,
) -> Option<IndexSignatureRecipe> {
    let ast = &captured.ast;
    let member = ast.node(id);
    let [parameter] = syntax::ids(member, "parameters") else {
        return None;
    };
    let parameter = ast.node(*parameter);
    let key = syntax::id(parameter, "type")?;
    Some(IndexSignatureRecipe {
        declaration: DeclarationIdentity::new(file, id),
        parameter_name: syntax::declaration_name_to_string(
            ast,
            syntax::id(parameter, "name"),
            captured.control.as_ref(),
        ),
        key_type: type_expression(inputs, file, captured, parents, key),
        value_type: syntax::id(member, "type")
            .map(|id| type_expression(inputs, file, captured, parents, id)),
        readonly: syntax::has_modifier(ast, member, SyntaxKind::ReadonlyKeyword),
    })
}

/// `getSymbolOfDeclaration` recognizes an object's direct property name as
/// its actual declaration value symbol. Exact
/// identifier and literal payloads come from the parser's existing name and
/// `parseLiteralLikeNode` owners; the raw own token remains its source root.
/// Computed-name expressions retain their expression question instead.
fn property_value_selection(
    file: FileId,
    ast: &Ast<'_>,
    parents: &[Option<NodeId>],
    id: NodeId,
) -> Result<Option<SelectedSyntax>, AnalysisError> {
    let Some(parent) = parents[id.index()] else {
        return Ok(None);
    };
    let property = ast.node(parent);
    if property.syntax_kind != SyntaxKind::PropertyAssignment
        || syntax::id(property, "name") != Some(id)
    {
        return Ok(None);
    }
    let name = ast.node(id);
    let key = match name.syntax_kind {
        SyntaxKind::Identifier => Some(
            ast.identifier_name(id)
                .ok_or_else(|| {
                    super::internal("direct property identifier has no exact parsed identity")
                })?
                .to_owned(),
        ),
        SyntaxKind::StringLiteral | SyntaxKind::NumericLiteral => {
            match property_names::scalar_name_text(ast, name) {
                Ok(Some(value)) => Some(value.to_owned()),
                Err(()) => None,
                _ => {
                    return Err(super::internal(
                        "direct property literal has no exact parsed payload",
                    ));
                }
            }
        }
        _ => return Ok(None),
    };
    if key.is_some() {
        return Ok(Some(SelectedSyntax::Declaration {
            anchor: ast.token_span(id),
            kind: DeclarationKind::Variable,
            declaration: DeclarationIdentity::new(file, parent),
        }));
    }
    // Preserve an unrepresentable own name until the value question follows
    // its checking prelude. No replacement key or symbol is fabricated.
    let reason = SemanticFailureReason::UnrepresentableIdentity;
    let message = "property name contains an unpaired UTF-16 surrogate";
    let token = ast.token_span(id);
    Ok(Some(SelectedSyntax::PendingValue {
        root: MemberRoot {
            file,
            start: token.start,
            end: token.end,
            role: MemberRole::PropertyAssignment,
        },
        name: key,
        reason,
        message: message.into(),
    }))
}

/// `getAliasSymbolForTypeNode`: parenthesized and readonly type operators
/// preserve the enclosing alias's identity; nested member type nodes do not.
fn alias_for_type_node(
    file: FileId,
    captured: &CapturedSyntax<'_>,
    parents: &[Option<NodeId>],
    id: NodeId,
) -> Option<AliasRecipe> {
    let ast = &captured.ast;
    let mut host = parents[id.index()]?;
    loop {
        let node = ast.node(host);
        if node.syntax_kind == SyntaxKind::ParenthesizedType
            || node.syntax_kind == SyntaxKind::TypeOperator
                && syntax::number(node, "operator") == Some(SyntaxKind::ReadonlyKeyword.number())
        {
            host = parents[host.index()]?;
        } else {
            break;
        }
    }
    let node = ast.node(host);
    if node.syntax_kind != SyntaxKind::TypeAliasDeclaration {
        return None;
    }
    syntax::id(node, "name")?;
    Some(AliasRecipe {
        declaration: DeclarationIdentity::new(file, host),
    })
}

fn intrinsic(kind: SyntaxKind) -> Option<PrimitiveName> {
    Some(match kind {
        SyntaxKind::StringKeyword => PrimitiveName::String,
        SyntaxKind::NumberKeyword => PrimitiveName::Number,
        SyntaxKind::BooleanKeyword => PrimitiveName::Boolean,
        SyntaxKind::BigIntKeyword => PrimitiveName::Bigint,
        SyntaxKind::SymbolKeyword => PrimitiveName::Symbol,
        SyntaxKind::ObjectKeyword => PrimitiveName::Object,
        _ => return None,
    })
}

fn special(kind: SyntaxKind) -> Option<SpecialName> {
    Some(match kind {
        SyntaxKind::AnyKeyword => SpecialName::Any,
        SyntaxKind::UnknownKeyword => SpecialName::Unknown,
        SyntaxKind::NeverKeyword => SpecialName::Never,
        SyntaxKind::VoidKeyword => SpecialName::Void,
        SyntaxKind::UndefinedKeyword => SpecialName::Undefined,
        SyntaxKind::NullKeyword => SpecialName::Null,
        _ => return None,
    })
}

/// Acquire expression descriptions from the final syntax. `parseLiteralLikeNode`
/// already owns cooked string and scanner-normalized numeric payloads; source
/// does not decode escapes or normalize a second raw spelling. Evaluation owns
/// `checkExpression` and literal freshness/type construction.
fn expression(
    inputs: CheckerInputs<'_>,
    file: FileId,
    captured: &CapturedSyntax<'_>,
    id: NodeId,
) -> Expression {
    let _depth = captured.resource_depth();
    let ast = &captured.ast;
    // Every captured expression retains its actual syntax, even when ordinary
    // checking is pending. Other native questions (for example getTypeOfDottedName)
    // can inspect that syntax without demanding its ordinary expression type.
    let mut expression = expression_worker(inputs, file, captured, id);
    expression.root = Some(ExpressionRoot {
        file,
        site: super::super::binding::flow::Site::new(ast, id),
    });
    Expression::new(expression)
}

fn expression_worker(
    inputs: CheckerInputs<'_>,
    file: FileId,
    captured: &CapturedSyntax<'_>,
    id: NodeId,
) -> ExpressionData {
    let ast = &captured.ast;
    let id = syntax::skip_parentheses(ast, id);
    let node = ast.node(id);
    let span = ast.token_span(id);
    let form = match node.syntax_kind {
        SyntaxKind::ThisKeyword => ExpressionForm::This(ExpressionRoot {
            file,
            site: super::super::binding::flow::Site::new(ast, id),
        }),
        SyntaxKind::SuperKeyword => ExpressionForm::Super(ExpressionRoot {
            file,
            site: super::super::binding::flow::Site::new(ast, id),
        }),
        SyntaxKind::ExpressionWithTypeArguments => ExpressionForm::Instantiation(ExpressionRoot {
            file,
            site: super::super::binding::flow::Site::new(ast, id),
        }),
        SyntaxKind::SpreadElement => ExpressionForm::Spread(ExpressionRoot {
            file,
            site: super::super::binding::flow::Site::new(ast, id),
        }),
        SyntaxKind::AwaitExpression => {
            let Some(operand) = syntax::id(node, "expression") else {
                return pending(
                    inputs,
                    file,
                    span,
                    SemanticFailureReason::UnsupportedSemanticForm,
                    "await expression has no operand",
                );
            };
            ExpressionForm::Await(Box::new(expression(inputs, file, captured, operand)))
        }
        SyntaxKind::TemplateExpression => ExpressionForm::Template(ExpressionRoot {
            file,
            site: super::super::binding::flow::Site::new(ast, id),
        }),
        SyntaxKind::TypeOfExpression => ExpressionForm::TypeOf(ExpressionRoot {
            file,
            site: super::super::binding::flow::Site::new(ast, id),
        }),
        SyntaxKind::BinaryExpression => ExpressionForm::Binary(ExpressionRoot {
            file,
            site: super::super::binding::flow::Site::new(ast, id),
        }),
        SyntaxKind::ConditionalExpression => ExpressionForm::ConditionalValue(ExpressionRoot {
            file,
            site: super::super::binding::flow::Site::new(ast, id),
        }),
        SyntaxKind::DeleteExpression => ExpressionForm::Delete(ExpressionRoot {
            file,
            site: super::super::binding::flow::Site::new(ast, id),
        }),
        SyntaxKind::FunctionExpression | SyntaxKind::ArrowFunction => {
            ExpressionForm::Function(ExpressionRoot {
                file,
                site: super::super::binding::flow::Site::new(ast, id),
            })
        }
        SyntaxKind::ClassExpression => ExpressionForm::Class(ExpressionRoot {
            file,
            site: super::super::binding::flow::Site::new(ast, id),
        }),
        SyntaxKind::ArrayLiteralExpression => ExpressionForm::Array(ExpressionRoot {
            file,
            site: super::super::binding::flow::Site::new(ast, id),
        }),
        SyntaxKind::AsExpression | SyntaxKind::TypeAssertionExpression => {
            ExpressionForm::Assertion(ExpressionRoot {
                file,
                site: super::super::binding::flow::Site::new(ast, id),
            })
        }
        SyntaxKind::NonNullExpression => ExpressionForm::NonNullAssertion(ExpressionRoot {
            file,
            site: super::super::binding::flow::Site::new(ast, id),
        }),
        SyntaxKind::SatisfiesExpression => ExpressionForm::Satisfies(ExpressionRoot {
            file,
            site: super::super::binding::flow::Site::new(ast, id),
        }),
        SyntaxKind::OmittedExpression => ExpressionForm::Omitted,
        SyntaxKind::PropertyAccessExpression
        | SyntaxKind::ElementAccessExpression
        | SyntaxKind::QualifiedName => ExpressionForm::Access(ExpressionRoot {
            file,
            site: super::super::binding::flow::Site::new(ast, id),
        }),
        SyntaxKind::CallExpression
        | SyntaxKind::NewExpression
        | SyntaxKind::TaggedTemplateExpression => {
            ExpressionForm::Call(CallRoot::new(file, ast.token_span(id), node.syntax_kind))
        }
        SyntaxKind::Identifier => ExpressionForm::Identifier {
            file,
            anchor: ast.token_span(id),
        },
        SyntaxKind::PrivateIdentifier => ExpressionForm::PrivateIdentifier(ExpressionRoot {
            file,
            site: super::super::binding::flow::Site::new(ast, id),
        }),
        SyntaxKind::RegularExpressionLiteral => ExpressionForm::RegularExpression,
        SyntaxKind::NullKeyword => ExpressionForm::Null,
        // `checkVoidExpression` schedules its operand for deferred checking;
        // it is not a value dependency of this type request.
        SyntaxKind::VoidExpression => ExpressionForm::Void,
        SyntaxKind::StringLiteral | SyntaxKind::NoSubstitutionTemplateLiteral => {
            match ast.cooked_literal(node) {
                Some(value) => ExpressionForm::Literal {
                    base: PrimitiveName::String,
                    value: captured.literal_value(value),
                },
                _ => {
                    return pending(
                        inputs,
                        file,
                        span,
                        SemanticFailureReason::UnsupportedSemanticForm,
                        "string literal has no exact cooked payload",
                    );
                }
            }
        }
        SyntaxKind::NumericLiteral => match node.field("text") {
            Some(Value::Text(value)) => ExpressionForm::Literal {
                base: PrimitiveName::Number,
                value: captured.literal_value(value.as_bytes()),
            },
            _ => {
                return pending(
                    inputs,
                    file,
                    span,
                    SemanticFailureReason::UnsupportedSemanticForm,
                    "numeric literal has no exact normalized payload",
                );
            }
        },
        SyntaxKind::BigIntLiteral => match node.field("text") {
            Some(Value::Text(value)) => ExpressionForm::Literal {
                base: PrimitiveName::Bigint,
                value: captured.literal_value(value.as_bytes()),
            },
            _ => {
                return pending(
                    inputs,
                    file,
                    span,
                    SemanticFailureReason::UnsupportedSemanticForm,
                    "bigint literal has no exact scanner payload",
                );
            }
        },
        SyntaxKind::TrueKeyword | SyntaxKind::FalseKeyword => ExpressionForm::Literal {
            base: PrimitiveName::Boolean,
            value: captured.literal_value(if node.syntax_kind == SyntaxKind::TrueKeyword {
                b"true"
            } else {
                b"false"
            }),
        },
        SyntaxKind::PrefixUnaryExpression | SyntaxKind::PostfixUnaryExpression => {
            let Some(operator) = syntax::unary_operator(node) else {
                return pending(
                    inputs,
                    file,
                    span,
                    SemanticFailureReason::UnsupportedSemanticForm,
                    "unary expression lacks a valid native operator",
                );
            };
            let Some(operand) = syntax::id(node, "operand") else {
                return pending(
                    inputs,
                    file,
                    span,
                    SemanticFailureReason::UnsupportedSemanticForm,
                    "unary expression has no operand",
                );
            };
            // checkPrefixUnaryExpression deliberately tests the direct syntax
            // kind. Parentheses must not acquire the literal shortcut merely
            // because recursive expression acquisition removes them.
            let direct_literal = match ast.node(operand).syntax_kind {
                SyntaxKind::NumericLiteral => Some(PrimitiveName::Number),
                SyntaxKind::BigIntLiteral => Some(PrimitiveName::Bigint),
                _ => None,
            };
            ExpressionForm::Unary {
                operator,
                direct_literal,
                operand: Box::new(expression(inputs, file, captured, operand)),
            }
        }
        // checkExpressionWorker dispatches all three JSX result forms to
        // JSX typing, which is explicitly outside this query profile.
        SyntaxKind::JsxElement | SyntaxKind::JsxSelfClosingElement | SyntaxKind::JsxFragment => {
            return pending(
                inputs,
                file,
                span,
                SemanticFailureReason::JsxTypingRequired,
                "JSX result types require deferred JSX typing",
            );
        }
        SyntaxKind::ObjectLiteralExpression => ExpressionForm::Object(ExpressionRoot {
            file,
            site: super::super::binding::flow::Site::new(ast, id),
        }),
        _ => {
            return pending(
                inputs,
                file,
                span,
                SemanticFailureReason::UnsupportedSemanticForm,
                "native expression operation is pending",
            );
        }
    };
    ExpressionData {
        location: super::location(inputs, file, span),
        root: None,
        form,
    }
}

fn pending(
    inputs: CheckerInputs<'_>,
    file: FileId,
    span: Span,
    reason: SemanticFailureReason,
    message: &str,
) -> ExpressionData {
    ExpressionData {
        location: super::location(inputs, file, span),
        root: None,
        form: ExpressionForm::Pending {
            reason,
            message: message.into(),
        },
    }
}

#[cfg(test)]
mod captured_syntax_tests {
    use super::CapturedSyntax;

    /// The attachment map is sized for the final tree, not for the parser's
    /// arena, which also holds the statements a top-level await reparse
    /// replaced: a file with the reparse gets the map of a file without it
    /// whose final tree is the same size.
    #[test]
    fn attachments_are_sized_for_the_final_tree() {
        let capture = |statement: &str| {
            let source = format!(
                "export class C {{ run() {{}} }}\nnew C().run();\n{}",
                statement.repeat(10_000)
            );
            let ast = crate::parse(&source).unwrap();
            let (reachable, arena) = (ast.walk().count(), ast.nodes().len());
            let captured = CapturedSyntax::new(ast, None).unwrap();
            (reachable, arena, captured.attachments.capacity())
        };
        let (reparsed, reparsed_arena, reparsed_capacity) = capture("await(0);\n");
        let (control, control_arena, control_capacity) = capture("void(0);\n");
        assert_eq!(reparsed, control, "both final trees have the same nodes");
        assert!(
            reparsed_arena > control_arena * 3 / 2,
            "the reparse leaves replaced nodes"
        );
        assert_eq!(reparsed_capacity, control_capacity);
    }
}
