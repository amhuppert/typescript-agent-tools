//! Transient native syntax questions used by flow construction. A postorder
//! pass evaluates their mutually dependent facts without a Rust call stack.

use crate::syntax as fields;
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, NodeId};

#[derive(Clone, Copy, Default)]
struct Fact {
    parent: Option<NodeId>,
    narrowable: bool,
    contains: bool,
    narrowing: bool,
    operand: bool,
    entity: bool,
    dotted: bool,
}

pub(in crate::project::binding) struct Facts(Vec<Fact>);

impl Facts {
    pub(in crate::project::binding) fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.0[id.index()].parent
    }

    pub(in crate::project::binding) fn new(ast: &Ast<'_>) -> Self {
        let mut facts = Self(vec![Fact::default(); ast.nodes().len()]);
        let order: Vec<_> = ast.walk().collect();
        for &id in &order {
            for child in ast.children(id) {
                facts.0[child.index()].parent = Some(id);
            }
        }
        for id in order.into_iter().rev() {
            let node = ast.node(id);
            let kind = node.syntax_kind;
            let expression = fields::id(node, "expression");
            let left = fields::id(node, "left");
            let right = fields::id(node, "right");
            let operator = fields::id(node, "operatorToken").map(|id| ast.node(id).syntax_kind);
            // `IsEntityNameExpressionEx(allowJS=false)`: entity names are a
            // narrower question than dotted call names or references.
            facts.0[id.index()].entity = kind == SyntaxKind::Identifier
                || kind == SyntaxKind::PropertyAccessExpression
                    && fields::id(node, "name")
                        .is_some_and(|name| ast.node(name).syntax_kind == SyntaxKind::Identifier)
                    && expression.is_some_and(|id| facts.0[id.index()].entity);
            // Reuse the native one-node decision while retaining the binder's
            // linear postorder evaluation of all dotted-name facts.
            facts.0[id.index()].dotted = match fields::dotted_name_step(ast, id) {
                fields::DottedNameStep::Result(result) => result,
                fields::DottedNameStep::Receiver(receiver) => facts.dotted(receiver),
            };
            // `isNarrowableReference`: the literal element-access branch
            // deliberately does not require a narrowable receiver.
            let narrowable = match kind {
                SyntaxKind::Identifier
                | SyntaxKind::ThisKeyword
                | SyntaxKind::SuperKeyword
                | SyntaxKind::MetaProperty => true,
                SyntaxKind::PropertyAccessExpression
                | SyntaxKind::ParenthesizedExpression
                | SyntaxKind::NonNullExpression => {
                    expression.is_some_and(|id| facts.narrowable(id))
                }
                SyntaxKind::ElementAccessExpression => fields::id(node, "argumentExpression")
                    .is_some_and(|argument| {
                        string_or_numeric_literal_like(ast.node(argument).syntax_kind)
                            || facts.0[argument.index()].entity
                                && expression.is_some_and(|id| facts.narrowable(id))
                    }),
                SyntaxKind::BinaryExpression => {
                    operator == Some(SyntaxKind::CommaToken)
                        && right.is_some_and(|id| facts.narrowable(id))
                        || fields::is_assignment_expression(ast, id, false)
                }
                _ => false,
            };
            facts.0[id.index()].narrowable = narrowable;
            // `containsNarrowableReference` adds optional-chain receivers.
            facts.0[id.index()].contains = narrowable
                || fields::is_optional_chain(ast, id)
                    && expression.is_some_and(|id| facts.contains(id));
            // `isNarrowableOperand`: assignment and comma select different
            // operands; it is not the general reference question above.
            facts.0[id.index()].operand = match kind {
                SyntaxKind::ParenthesizedExpression => {
                    expression.is_some_and(|id| facts.operand(id))
                }
                SyntaxKind::BinaryExpression if operator == Some(SyntaxKind::EqualsToken) => {
                    left.is_some_and(|id| facts.operand(id))
                }
                SyntaxKind::BinaryExpression if operator == Some(SyntaxKind::CommaToken) => {
                    right.is_some_and(|id| facts.operand(id))
                }
                _ => facts.contains(id),
            };
            // `isNarrowingExpression` / `isNarrowingBinaryExpression` /
            // `hasNarrowableArgument` consume the completed child facts.
            facts.0[id.index()].narrowing = match kind {
                SyntaxKind::Identifier | SyntaxKind::ThisKeyword => true,
                SyntaxKind::PropertyAccessExpression | SyntaxKind::ElementAccessExpression => {
                    facts.contains(id)
                }
                SyntaxKind::CallExpression => {
                    fields::ids(node, "arguments")
                        .iter()
                        .any(|&id| facts.contains(id))
                        || expression.is_some_and(|callee| {
                            ast.node(callee).syntax_kind == SyntaxKind::PropertyAccessExpression
                                && fields::id(ast.node(callee), "expression")
                                    .is_some_and(|id| facts.contains(id))
                        })
                }
                SyntaxKind::ParenthesizedExpression
                | SyntaxKind::NonNullExpression
                | SyntaxKind::TypeOfExpression => expression.is_some_and(|id| facts.narrowing(id)),
                SyntaxKind::PrefixUnaryExpression => {
                    fields::number(node, "operator") == Some(SyntaxKind::ExclamationToken.number())
                        && fields::id(node, "operand").is_some_and(|id| facts.narrowing(id))
                }
                SyntaxKind::BinaryExpression => match operator {
                    Some(
                        SyntaxKind::EqualsToken
                        | SyntaxKind::BarBarEqualsToken
                        | SyntaxKind::AmpersandAmpersandEqualsToken
                        | SyntaxKind::QuestionQuestionEqualsToken,
                    ) => left.is_some_and(|id| facts.contains(id)),
                    Some(
                        SyntaxKind::EqualsEqualsToken
                        | SyntaxKind::ExclamationEqualsToken
                        | SyntaxKind::EqualsEqualsEqualsToken
                        | SyntaxKind::ExclamationEqualsEqualsToken,
                    ) => match (left, right) {
                        (Some(left), Some(right)) => {
                            let left = fields::skip_parentheses(ast, left);
                            let right = fields::skip_parentheses(ast, right);
                            facts.operand(left)
                                || facts.operand(right)
                                || facts.typeof_operands(ast, right, left)
                                || facts.typeof_operands(ast, left, right)
                                || boolean_literal(ast.node(right).syntax_kind)
                                    && facts.narrowing(left)
                                || boolean_literal(ast.node(left).syntax_kind)
                                    && facts.narrowing(right)
                        }
                        _ => false,
                    },
                    Some(SyntaxKind::InstanceOfKeyword) => left.is_some_and(|id| facts.operand(id)),
                    Some(SyntaxKind::InKeyword | SyntaxKind::CommaToken) => {
                        right.is_some_and(|id| facts.narrowing(id))
                    }
                    _ => false,
                },
                _ => false,
            };
        }
        facts
    }

    pub(super) fn assignment_target(&self, ast: &Ast<'_>, id: NodeId) -> Option<NodeId> {
        fields::assignment_target(ast, |id| self.0[id.index()].parent, id)
    }

    fn typeof_operands(&self, ast: &Ast<'_>, first: NodeId, second: NodeId) -> bool {
        ast.node(first).syntax_kind == SyntaxKind::TypeOfExpression
            && fields::id(ast.node(first), "expression").is_some_and(|id| self.operand(id))
            && string_literal_like(ast.node(second).syntax_kind)
    }
    pub(super) fn narrowable(&self, id: NodeId) -> bool {
        self.0[id.index()].narrowable
    }
    pub(super) fn contains(&self, id: NodeId) -> bool {
        self.0[id.index()].contains
    }
    pub(super) fn narrowing(&self, id: NodeId) -> bool {
        self.0[id.index()].narrowing
    }
    pub(super) fn operand(&self, id: NodeId) -> bool {
        self.0[id.index()].operand
    }
    pub(super) fn dotted(&self, id: NodeId) -> bool {
        self.0[id.index()].dotted
    }

    /// `IsLogicalExpression`: parentheses and logical negation unwrap;
    /// logical assignment is deliberately a different native question.
    pub(super) fn logical_expression(&self, ast: &Ast<'_>, mut id: NodeId) -> bool {
        loop {
            let node = ast.node(id);
            match node.syntax_kind {
                SyntaxKind::ParenthesizedExpression => {
                    id = fields::id(node, "expression")
                        .expect("parenthesized expression has its child");
                }
                SyntaxKind::PrefixUnaryExpression
                    if fields::number(node, "operator")
                        == Some(SyntaxKind::ExclamationToken.number()) =>
                {
                    id = fields::id(node, "operand").expect("logical negation has its operand");
                }
                _ => {
                    return node.syntax_kind == SyntaxKind::BinaryExpression
                        && fields::id(node, "operatorToken").is_some_and(|operator| {
                            matches!(
                                ast.node(operator).syntax_kind,
                                SyntaxKind::AmpersandAmpersandToken
                                    | SyntaxKind::BarBarToken
                                    | SyntaxKind::QuestionQuestionToken
                            )
                        });
                }
            }
        }
    }

    /// Binder `isLogicalAssignmentExpression`: only parentheses unwrap.
    pub(super) fn logical_assignment(&self, ast: &Ast<'_>, id: NodeId) -> bool {
        let node = ast.node(fields::skip_parentheses(ast, id));
        node.syntax_kind == SyntaxKind::BinaryExpression
            && fields::id(node, "operatorToken").is_some_and(|operator| {
                matches!(
                    ast.node(operator).syntax_kind,
                    SyntaxKind::AmpersandAmpersandEqualsToken
                        | SyntaxKind::BarBarEqualsToken
                        | SyntaxKind::QuestionQuestionEqualsToken
                )
            })
    }

    /// `isTopLevelLogicalExpression` / `isStatementCondition`: exact parent
    /// roles determine whether this expression owns its post-expression join.
    pub(super) fn top_level_logical(&self, ast: &Ast<'_>, mut id: NodeId) -> bool {
        while let Some(parent) = self.parent(id) {
            let node = ast.node(parent);
            if node.syntax_kind == SyntaxKind::ParenthesizedExpression
                || node.syntax_kind == SyntaxKind::PrefixUnaryExpression
                    && fields::number(node, "operator")
                        == Some(SyntaxKind::ExclamationToken.number())
            {
                id = parent;
            } else {
                break;
            }
        }
        let Some(parent) = self.parent(id) else {
            return true;
        };
        let node = ast.node(parent);
        let statement_condition = match node.syntax_kind {
            SyntaxKind::IfStatement | SyntaxKind::WhileStatement | SyntaxKind::DoStatement => {
                fields::id(node, "expression") == Some(id)
            }
            SyntaxKind::ForStatement | SyntaxKind::ConditionalExpression => {
                fields::id(node, "condition") == Some(id)
            }
            _ => false,
        };
        !(statement_condition
            || self.logical_expression(ast, parent)
            || fields::is_optional_chain(ast, parent) && fields::id(node, "expression") == Some(id))
    }

    /// `IsOutermostOptionalChain`: an argument, new root, parentheses or a
    /// non-chain parent ends this exact optional expression.
    pub(super) fn outermost_optional(&self, ast: &Ast<'_>, id: NodeId) -> bool {
        fields::is_outermost_optional_chain(ast, id, self.parent(id))
    }

    /// `IsOptionalChainRoot`: NonNull can be a chain but is never its root.
    pub(super) fn optional_root(&self, ast: &Ast<'_>, id: NodeId) -> bool {
        fields::is_optional_chain_root(ast, id)
    }

    /// `IsNullishCoalesce`: createFlowCondition asks about the immediate
    /// parent, rather than every logical ancestor of a literal.
    pub(super) fn nullish_parent(&self, ast: &Ast<'_>, id: NodeId) -> bool {
        self.0[id.index()].parent.is_some_and(|parent| {
            let node = ast.node(parent);
            node.syntax_kind == SyntaxKind::BinaryExpression
                && fields::id(node, "operatorToken")
                    .is_some_and(|id| ast.node(id).syntax_kind == SyntaxKind::QuestionQuestionToken)
        })
    }

    /// `GetContainerFlags`: the complete native container classification for
    /// this project's TS/TSX grammar. Native JSTypeAliasDeclaration is outside
    /// that accepted grammar and has no parser-schema node here.
    pub(in crate::project::binding) fn container_flags(
        &self,
        ast: &Ast<'_>,
        id: NodeId,
    ) -> ContainerFlags {
        use ContainerFlags as C;
        let node = ast.node(id);
        let flags = match node.syntax_kind {
            SyntaxKind::ClassExpression
            | SyntaxKind::ClassDeclaration
            | SyntaxKind::EnumDeclaration
            | SyntaxKind::ObjectLiteralExpression
            | SyntaxKind::TypeLiteral
            | SyntaxKind::JsxAttributes => C::CONTAINER,
            SyntaxKind::InterfaceDeclaration => C::CONTAINER | C::INTERFACE,
            SyntaxKind::ModuleDeclaration
            | SyntaxKind::TypeAliasDeclaration
            | SyntaxKind::MappedType
            | SyntaxKind::IndexSignature => C::CONTAINER | C::LOCALS,
            SyntaxKind::SourceFile => C::CONTAINER | C::CONTROL_FLOW | C::LOCALS,
            SyntaxKind::GetAccessor | SyntaxKind::SetAccessor | SyntaxKind::MethodDeclaration => {
                let special_method = self.parent(id).is_some_and(|parent| {
                    matches!(
                        ast.node(parent).syntax_kind,
                        SyntaxKind::ObjectLiteralExpression | SyntaxKind::ClassExpression
                    )
                });
                C::CONTAINER
                    | C::CONTROL_FLOW
                    | C::LOCALS
                    | C::FUNCTION_LIKE
                    | C::THIS_CONTAINER
                    | if special_method {
                        C::CAPTURING_METHOD
                    } else {
                        0
                    }
            }
            SyntaxKind::Constructor
            | SyntaxKind::FunctionDeclaration
            | SyntaxKind::ClassStaticBlockDeclaration => {
                C::CONTAINER | C::CONTROL_FLOW | C::LOCALS | C::FUNCTION_LIKE | C::THIS_CONTAINER
            }
            SyntaxKind::MethodSignature
            | SyntaxKind::CallSignature
            | SyntaxKind::FunctionType
            | SyntaxKind::ConstructSignature
            | SyntaxKind::ConstructorType => {
                C::CONTAINER | C::CONTROL_FLOW | C::LOCALS | C::FUNCTION_LIKE | C::PROPAGATES_THIS
            }
            SyntaxKind::FunctionExpression => {
                C::CONTAINER
                    | C::CONTROL_FLOW
                    | C::LOCALS
                    | C::FUNCTION_LIKE
                    | C::FUNCTION_EXPRESSION
                    | C::THIS_CONTAINER
            }
            SyntaxKind::ArrowFunction => {
                C::CONTAINER
                    | C::CONTROL_FLOW
                    | C::LOCALS
                    | C::FUNCTION_LIKE
                    | C::FUNCTION_EXPRESSION
                    | C::PROPAGATES_THIS
            }
            SyntaxKind::ModuleBlock => C::CONTROL_FLOW,
            SyntaxKind::PropertyDeclaration => {
                if fields::id(node, "initializer").is_some() {
                    C::CONTROL_FLOW | C::THIS_CONTAINER
                } else {
                    0
                }
            }
            SyntaxKind::CatchClause
            | SyntaxKind::ForStatement
            | SyntaxKind::ForInStatement
            | SyntaxKind::ForOfStatement
            | SyntaxKind::CaseBlock => C::BLOCK_SCOPED | C::LOCALS,
            SyntaxKind::Block => {
                if self.parent(id).is_some_and(|parent| {
                    fields::is_function_like_kind(ast.node(parent).syntax_kind)
                        || ast.node(parent).syntax_kind == SyntaxKind::ClassStaticBlockDeclaration
                }) {
                    0
                } else {
                    C::BLOCK_SCOPED | C::LOCALS
                }
            }
            _ => 0,
        };
        ContainerFlags(flags)
    }

    /// `GetImmediatelyInvokedFunctionExpression` plus bindContainer's async,
    /// generator and static-block execution conditions.
    pub(super) fn immediate(&self, ast: &Ast<'_>, id: NodeId) -> bool {
        let node = ast.node(id);
        if node.syntax_kind == SyntaxKind::ClassStaticBlockDeclaration {
            return true;
        }
        let function_flags = fields::function_flags(ast, id);
        if !self.container_flags(ast, id).function_expression()
            || function_flags.is_async()
            || function_flags.is_generator()
        {
            return false;
        }
        let mut previous = id;
        let mut parent = self.0[id.index()].parent;
        while let Some(id) = parent {
            if ast.node(id).syntax_kind != SyntaxKind::ParenthesizedExpression {
                break;
            }
            previous = id;
            parent = self.0[id.index()].parent;
        }
        parent.is_some_and(|parent| {
            ast.node(parent).syntax_kind == SyntaxKind::CallExpression
                && fields::id(ast.node(parent), "expression") == Some(previous)
        })
    }
}

/// Native ContainerFlags are distinct questions over one established result.
#[derive(Clone, Copy)]
pub(in crate::project::binding) struct ContainerFlags(u16);

impl ContainerFlags {
    const CONTAINER: u16 = 1;
    const BLOCK_SCOPED: u16 = 1 << 1;
    const CONTROL_FLOW: u16 = 1 << 2;
    const FUNCTION_LIKE: u16 = 1 << 3;
    const FUNCTION_EXPRESSION: u16 = 1 << 4;
    const LOCALS: u16 = 1 << 5;
    const INTERFACE: u16 = 1 << 6;
    const CAPTURING_METHOD: u16 = 1 << 7;
    const THIS_CONTAINER: u16 = 1 << 8;
    const PROPAGATES_THIS: u16 = 1 << 9;

    /// `IsContainer`: the binder's `container` and `blockScopeContainer`.
    pub(in crate::project::binding) fn container(self) -> bool {
        self.0 & Self::CONTAINER != 0
    }
    /// `IsBlockScopedContainer`: the binder's `blockScopeContainer` alone.
    pub(in crate::project::binding) fn block_scoped(self) -> bool {
        self.0 & Self::BLOCK_SCOPED != 0
    }
    /// `IsThisContainer`: the binder's `thisContainer`.
    pub(in crate::project::binding) fn this_container(self) -> bool {
        self.0 & Self::THIS_CONTAINER != 0
    }
    pub(super) fn control_flow(self) -> bool {
        self.0 & Self::CONTROL_FLOW != 0
    }
    pub(super) fn interface(self) -> bool {
        self.0 & Self::INTERFACE != 0
    }
    pub(super) fn propagates_this(self) -> bool {
        self.0 & Self::PROPAGATES_THIS != 0
    }
    pub(super) fn captures_outer_flow(self) -> bool {
        self.0 & (Self::FUNCTION_EXPRESSION | Self::CAPTURING_METHOD) != 0
    }
    fn function_expression(self) -> bool {
        self.0 & Self::FUNCTION_EXPRESSION != 0
    }
}

/// `IsStringLiteralLike` / `IsStringOrNumericLiteralLike` are separate exact
/// kind questions; bigint is not a numeric-literal-like element index.
fn string_literal_like(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::StringLiteral | SyntaxKind::NoSubstitutionTemplateLiteral
    )
}
fn string_or_numeric_literal_like(kind: SyntaxKind) -> bool {
    string_literal_like(kind) || kind == SyntaxKind::NumericLiteral
}
fn boolean_literal(kind: SyntaxKind) -> bool {
    matches!(kind, SyntaxKind::TrueKeyword | SyntaxKind::FalseKeyword)
}

/// `IsLogicalOrCoalescingBinaryOperator` / `IsLogicalOrCoalescingAssignmentOperator`.
pub(super) fn is_logical_operator(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::BarBarToken
            | SyntaxKind::AmpersandAmpersandToken
            | SyntaxKind::QuestionQuestionToken
            | SyntaxKind::BarBarEqualsToken
            | SyntaxKind::AmpersandAmpersandEqualsToken
            | SyntaxKind::QuestionQuestionEqualsToken
    )
}
