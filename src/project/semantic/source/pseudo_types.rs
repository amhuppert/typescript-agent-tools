//! Native PseudoChecker syntax recipes. These are unnormalized syntax, not
//! checker types: creation never resolves a reference or checks an expression.

use super::super::model::DemandError;
use super::{DeclarationIdentity as N, ExpressionRoot, Source, TypeExpression};
use crate::project::binding::{ReadOrigin, ReferenceSymbol};
use crate::{syntax, syntax_kind::SyntaxKind as K};

#[derive(Clone)]
pub(in crate::project::semantic) enum PseudoType {
    Direct(N),
    Inferred {
        expression: N,
        signature_return: bool,
        errors: Vec<N>,
    },
    NoResult(N),
    MaybeConst {
        node: N,
        constant: Box<Self>,
        regular: Box<Self>,
    },
    Union(Vec<Self>),
    Atom(K),
    Literal {
        node: N,
    },
    Function(PseudoSignature),
    Tuple(Vec<Self>),
    Object(Vec<PseudoMember>),
}
#[derive(Clone)]
pub(in crate::project::semantic) struct PseudoSignature {
    pub(in crate::project::semantic) declaration: N,
    pub(in crate::project::semantic) parameters: Vec<PseudoParameter>,
    pub(in crate::project::semantic) type_parameters: Vec<N>,
    pub(in crate::project::semantic) returns: Box<PseudoType>,
}
#[derive(Clone)]
pub(in crate::project::semantic) struct PseudoParameter {
    pub(in crate::project::semantic) rest: bool,
    pub(in crate::project::semantic) name: N,
    pub(in crate::project::semantic) optional: bool,
    pub(in crate::project::semantic) ty: PseudoType,
}
#[derive(Clone)]
pub(in crate::project::semantic) struct PseudoMember {
    pub(in crate::project::semantic) name: N,
    pub(in crate::project::semantic) optional: bool,
    pub(in crate::project::semantic) kind: PseudoMemberKind,
}
#[derive(Clone)]
pub(in crate::project::semantic) enum PseudoMemberKind {
    Property {
        readonly: bool,
        ty: PseudoType,
    },
    Method(PseudoSignature),
    Get {
        declaration: N,
        ty: PseudoType,
    },
    Set {
        declaration: N,
        parameter: PseudoParameter,
    },
}
struct Accessors {
    first: N,
    second: Option<N>,
    get: Option<N>,
    set: Option<N>,
}

impl PseudoType {
    fn inferred(expression: N, signature_return: bool) -> Self {
        Self::Inferred {
            expression,
            signature_return,
            errors: Vec::new(),
        }
    }
    fn usable_initializer(&self) -> bool {
        !matches!(self, Self::Inferred { errors, .. } if errors.is_empty())
    }
    fn maybe_const(node: N, constant: Self, regular: K) -> Self {
        Self::MaybeConst {
            node,
            constant: Box::new(constant),
            regular: Box::new(Self::Atom(regular)),
        }
    }
}

impl Source<'_, '_> {
    /// `PseudoChecker.GetTypeOfDeclaration` keeps native NoResult distinct from
    /// a missing producer or resource interruption.
    pub(in crate::project::semantic) fn pseudo_declaration_type(
        &mut self,
        node: N,
    ) -> Result<PseudoType, DemandError> {
        let _depth = self
            .control
            .as_ref()
            .map(|control| control.enter())
            .transpose()
            .map_err(super::super::resource_failure)?;
        let kind = self.declaration_kind(node)?;
        if kind == K::Parameter {
            return self.pseudo_parameter_type(node, None);
        }
        if let Some(annotation) = self.pseudo_child(node, "type")? {
            return Ok(PseudoType::Direct(annotation));
        }
        match kind {
            K::VariableDeclaration => {
                if let Some(initializer) = self.pseudo_child(node, "initializer")? {
                    let declarations = self.pseudo_raw_declarations(node)?;
                    let mut variables = 0;
                    for &declaration in &declarations {
                        variables += usize::from(
                            self.declaration_kind(declaration)? == K::VariableDeclaration,
                        );
                    }
                    if (declarations.len() == 1 || variables == 1)
                        && !self.pseudo_contextually_typed(node)?
                    {
                        let constant = {
                            let captured = self.syntax(node.file)?;
                            captured.parents[node.node.index()].is_some_and(|parent| {
                                syntax::is_var_const_flags(captured.ast.node(parent).flags)
                            })
                        };
                        if !(constant
                            && self.declaration_kind(initializer)? == K::TemplateExpression)
                        {
                            let ty = self.pseudo_expression_type(initializer)?;
                            if ty.usable_initializer() {
                                return Ok(ty);
                            }
                        }
                    }
                }
            }
            K::PropertyDeclaration => {
                if let Some(initializer) = self.pseudo_child(node, "initializer")?
                    && !self.pseudo_contextually_typed(node)?
                    && !(self.declaration_has_modifier(node, K::ReadonlyKeyword)?
                        && self.declaration_kind(initializer)? == K::TemplateExpression)
                {
                    let ty = self.pseudo_expression_type(initializer)?;
                    if ty.usable_initializer() {
                        return if !matches!(ty, PseudoType::Direct(_))
                            && self.pseudo_child(node, "questionToken")?.is_some()
                        {
                            self.pseudo_add_undefined(ty)
                        } else {
                            Ok(ty)
                        };
                    }
                }
            }
            K::PropertyAssignment => {
                if let Some(initializer) = self.pseudo_child(node, "initializer")? {
                    let ty = self.pseudo_expression_type(initializer)?;
                    if ty.usable_initializer() {
                        return Ok(ty);
                    }
                }
            }
            K::ExportAssignment => {
                let expression = self.pseudo_required_child(node, "expression")?;
                return self.pseudo_expression_type(expression);
            }
            K::BindingElement
            | K::PropertySignature
            | K::ShorthandPropertyAssignment
            | K::PropertyAccessExpression
            | K::ElementAccessExpression
            | K::BinaryExpression
            | K::CallExpression => {}
            _ => {
                return Err(super::super::internal(
                    "PseudoChecker declaration has another native syntax kind",
                )
                .into());
            }
        }
        Ok(PseudoType::NoResult(node))
    }

    /// `PseudoChecker.typeFromExpression` uses only captured grammar and the
    /// separate syntactic const/context predicates.
    pub(in crate::project::semantic) fn pseudo_expression_type(
        &mut self,
        node: N,
    ) -> Result<PseudoType, DemandError> {
        let _depth = self
            .control
            .as_ref()
            .map(|control| control.enter())
            .transpose()
            .map_err(super::super::resource_failure)?;
        let kind = self.declaration_kind(node)?;
        let primitive =
            |regular| PseudoType::maybe_const(node, PseudoType::Literal { node }, regular);
        Ok(match kind {
            K::OmittedExpression => PseudoType::Atom(K::UndefinedKeyword),
            K::ParenthesizedExpression => {
                let expression = self.pseudo_required_child(node, "expression")?;
                self.pseudo_expression_type(expression)?
            }
            K::Identifier
                if self.syntax(node.file)?.ast.identifier_name(node.node) == Some("undefined") =>
            {
                PseudoType::Atom(K::UndefinedKeyword)
            }
            K::NullKeyword => PseudoType::Atom(K::NullKeyword),
            K::ArrowFunction | K::FunctionExpression => {
                PseudoType::Function(self.pseudo_signature(node)?)
            }
            K::AsExpression | K::TypeAssertionExpression => {
                let assertion = self.pseudo_required_child(node, "type")?;
                if syntax::is_const_type_reference(&self.syntax(node.file)?.ast, assertion.node) {
                    let expression = self.pseudo_required_child(node, "expression")?;
                    self.pseudo_expression_type(expression)?
                } else {
                    PseudoType::Direct(assertion)
                }
            }
            K::PrefixUnaryExpression if self.pseudo_primitive_literal(node, true)? => {
                let operand = self.pseudo_required_child(node, "operand")?;
                let plus = syntax::number(self.syntax(node.file)?.ast.node(node.node), "operator")
                    == Some(K::PlusToken as u32);
                let literal = if plus { operand } else { node };
                let bigint = self.declaration_kind(operand)? == K::BigIntLiteral;
                PseudoType::maybe_const(
                    node,
                    PseudoType::Literal { node: literal },
                    if bigint {
                        K::BigIntKeyword
                    } else {
                        K::NumberKeyword
                    },
                )
            }
            K::ArrayLiteralExpression => self.pseudo_array(node)?,
            K::ObjectLiteralExpression => self.pseudo_object(node)?,
            K::ClassExpression => PseudoType::Inferred {
                expression: node,
                signature_return: false,
                errors: vec![node],
            },
            K::TemplateExpression => {
                let inferred = PseudoType::inferred(node, false);
                if self.pseudo_is_in_const_context(node)? {
                    inferred
                } else {
                    PseudoType::maybe_const(node, inferred, K::StringKeyword)
                }
            }
            K::NumericLiteral => primitive(K::NumberKeyword),
            K::BigIntLiteral => primitive(K::BigIntKeyword),
            K::StringLiteral | K::NoSubstitutionTemplateLiteral => primitive(K::StringKeyword),
            K::TrueKeyword | K::FalseKeyword => {
                PseudoType::maybe_const(node, PseudoType::Atom(kind), K::BooleanKeyword)
            }
            _ => PseudoType::inferred(node, false),
        })
    }

    /// `GetReturnTypeOfSignature` delegates getters to their actual raw pair.
    pub(in crate::project::semantic) fn pseudo_return_type(
        &mut self,
        node: N,
    ) -> Result<PseudoType, DemandError> {
        if self.declaration_kind(node)? == K::GetAccessor {
            self.pseudo_accessor_type(node)
        } else {
            self.pseudo_create_return(node)
        }
    }
    fn pseudo_create_return(&mut self, node: N) -> Result<PseudoType, DemandError> {
        if let Some(annotation) = self.pseudo_child(node, "type")? {
            return Ok(PseudoType::Direct(annotation));
        }
        if !matches!(
            self.declaration_kind(node)?,
            K::FunctionExpression
                | K::ArrowFunction
                | K::MethodDeclaration
                | K::GetAccessor
                | K::SetAccessor
                | K::FunctionDeclaration
                | K::Constructor
        ) {
            return Ok(PseudoType::NoResult(node));
        }
        let candidate = {
            let captured = self.syntax(node.file)?;
            let ast = &captured.ast;
            let flags = syntax::function_flags(ast, node.node);
            let mut candidate = None;
            if let Some(body) = syntax::id(ast.node(node.node), "body") {
                if !flags.is_async() && !flags.is_generator() {
                    if ast.node(body).syntax_kind == K::Block {
                        for statement in syntax::return_statements(ast, body) {
                            if captured.parents[statement.index()] != Some(body)
                                || candidate.is_some()
                            {
                                candidate = None;
                                break;
                            }
                            candidate = syntax::id(ast.node(statement), "expression");
                        }
                    } else {
                        candidate = Some(body);
                    }
                }
            }
            candidate.map(|id| N::new(node.file, id))
        };
        if let Some(candidate) = candidate {
            if !self.pseudo_contextually_typed(candidate)? {
                return self.pseudo_expression_type(candidate);
            }
            if matches!(
                self.declaration_kind(candidate)?,
                K::AsExpression | K::TypeAssertionExpression
            ) {
                let annotation = self.pseudo_required_child(candidate, "type")?;
                if !syntax::is_const_type_reference(&self.syntax(node.file)?.ast, annotation.node) {
                    return Ok(PseudoType::Direct(annotation));
                }
            }
        }
        Ok(PseudoType::inferred(node, true))
    }

    pub(in crate::project::semantic) fn pseudo_accessor_type(
        &mut self,
        node: N,
    ) -> Result<PseudoType, DemandError> {
        let accessors = self.pseudo_accessors(node)?;
        let mut annotation = self.pseudo_accessor_annotation(node)?;
        if annotation.is_none() && node != accessors.first {
            annotation = self.pseudo_accessor_annotation(accessors.first)?;
        }
        if annotation.is_none()
            && let Some(second) = accessors.second
            && node != second
        {
            annotation = self.pseudo_accessor_annotation(second)?;
        }
        if let Some(annotation) = annotation
            && self.declaration_kind(annotation)? != K::TypePredicate
        {
            return Ok(PseudoType::Direct(annotation));
        }
        if let Some(getter) = accessors.get {
            let mut result = self.pseudo_create_return(getter)?;
            if let PseudoType::Inferred { errors, .. } = &mut result
                && errors.is_empty()
            {
                errors.push(getter);
                if let Some(setter) = accessors.set {
                    errors.push(setter);
                }
            }
            return Ok(result);
        }
        Ok(PseudoType::NoResult(node))
    }
    fn pseudo_accessor_annotation(&mut self, node: N) -> Result<Option<N>, DemandError> {
        if self.declaration_kind(node)? == K::GetAccessor {
            self.pseudo_child(node, "type")
        } else {
            let parameters = self.pseudo_children(node, "parameters")?;
            match parameters.first().copied() {
                Some(parameter) if self.declaration_kind(parameter)? == K::Parameter => {
                    self.pseudo_child(parameter, "type")
                }
                _ => Ok(None),
            }
        }
    }
    /// `GetAllAccessorDeclarationsForDeclaration` uses the first opposite raw
    /// declaration, then compares native positions; it does not merge symbols.
    fn pseudo_accessors(&mut self, node: N) -> Result<Accessors, DemandError> {
        let is_get = self.declaration_kind(node)? == K::GetAccessor;
        let opposite = if is_get {
            K::SetAccessor
        } else {
            K::GetAccessor
        };
        let mut other = None;
        for declaration in self.pseudo_raw_declarations(node)? {
            if self.declaration_kind(declaration)? == opposite {
                other = Some(declaration);
                break;
            }
        }
        let before = if let Some(other) = other {
            let pos = self.syntax(node.file)?.ast.node(node.node).span.start;
            self.syntax(other.file)?.ast.node(other.node).span.start < pos
        } else {
            false
        };
        Ok(Accessors {
            first: if before {
                other.expect("opposite accessor")
            } else {
                node
            },
            second: if before { Some(node) } else { other },
            get: if is_get { Some(node) } else { other },
            set: if is_get { other } else { Some(node) },
        })
    }

    fn pseudo_signature(&mut self, node: N) -> Result<PseudoSignature, DemandError> {
        // Native FullSignature is a JS-only reparsing product; TS inputs have
        // their ordinary signature fields and never manufacture one here.
        let returns = Box::new(self.pseudo_create_return(node)?);
        let type_parameters = self.pseudo_children(node, "typeParameters")?;
        let parameters = self.pseudo_parameters(node)?;
        Ok(PseudoSignature {
            declaration: node,
            parameters,
            type_parameters,
            returns,
        })
    }
    fn pseudo_parameters(&mut self, node: N) -> Result<Vec<PseudoParameter>, DemandError> {
        let parameters = self.pseudo_children(node, "parameters")?;
        let last_required = self.pseudo_last_required(&parameters)?;
        let mut result = Vec::new();
        for (i, parameter) in parameters.into_iter().enumerate() {
            let optional = self.pseudo_child(parameter, "questionToken")?.is_some()
                || self.pseudo_child(parameter, "initializer")?.is_some() && i + 1 >= last_required;
            result.push(PseudoParameter {
                name: self.pseudo_required_child(parameter, "name")?,
                rest: self.pseudo_child(parameter, "dotDotDotToken")?.is_some(),
                optional,
                ty: self.pseudo_parameter_type(parameter, Some((i, last_required)))?,
            });
        }
        Ok(result)
    }
    fn pseudo_last_required(&mut self, parameters: &[N]) -> Result<usize, DemandError> {
        for (i, &parameter) in parameters.iter().enumerate().rev() {
            if self.pseudo_child(parameter, "questionToken")?.is_none()
                && self.pseudo_child(parameter, "initializer")?.is_none()
                && self.pseudo_child(parameter, "dotDotDotToken")?.is_none()
            {
                return Ok(i + 1);
            }
        }
        Ok(0)
    }
    fn pseudo_parameter_type(
        &mut self,
        node: N,
        position: Option<(usize, usize)>,
    ) -> Result<PseudoType, DemandError> {
        let parent = self
            .declaration_parent(node)?
            .ok_or_else(|| super::super::internal("parameter has no parent"))?;
        if self.declaration_kind(parent)? == K::SetAccessor {
            return self.pseudo_accessor_type(parent);
        }
        let initializer = self.pseudo_child(node, "initializer")?;
        let annotation = self.pseudo_child(node, "type")?;
        if position.is_none() && initializer.is_none() {
            return Ok(annotation
                .map(PseudoType::Direct)
                .unwrap_or(PseudoType::NoResult(node)));
        }
        let (index, last_required) = match position {
            Some(position) => position,
            None => {
                let parameters = self.pseudo_children(parent, "parameters")?;
                let index = parameters
                    .iter()
                    .position(|&parameter| parameter == node)
                    .ok_or_else(|| {
                        super::super::internal("parameter absent from its parent's list")
                    })?;
                (index, self.pseudo_last_required(&parameters)?)
            }
        };
        let add_undefined = self.inputs.semantic_options().strict_null_checks
            && initializer.is_some()
            && index + 1 < last_required;
        if let Some(annotation) = annotation {
            let ty = PseudoType::Direct(annotation);
            return if add_undefined {
                self.pseudo_add_undefined(ty)
            } else {
                Ok(ty)
            };
        }
        if let Some(initializer) = initializer {
            let name = self.pseudo_required_child(node, "name")?;
            if self.declaration_kind(name)? == K::Identifier
                && !self.pseudo_contextually_typed(node)?
            {
                let mut ty = self.pseudo_expression_type(initializer)?;
                if let PseudoType::Inferred {
                    errors,
                    signature_return,
                    ..
                } = &mut ty
                    && errors.is_empty()
                {
                    errors.push(node);
                    *signature_return = false;
                }
                return if add_undefined {
                    self.pseudo_add_undefined(ty)
                } else {
                    Ok(ty)
                };
            }
        }
        Ok(PseudoType::NoResult(node))
    }

    fn pseudo_array(&mut self, node: N) -> Result<PseudoType, DemandError> {
        if !self.pseudo_is_in_const_context(node)? {
            return Ok(PseudoType::Inferred {
                expression: node,
                signature_return: false,
                errors: vec![node],
            });
        }
        let elements = self.pseudo_children(node, "elements")?;
        for &element in &elements {
            if self.declaration_kind(element)? == K::SpreadElement {
                return Ok(PseudoType::Inferred {
                    expression: node,
                    signature_return: false,
                    errors: vec![element],
                });
            }
        }
        if self.pseudo_contextually_typed(node)? {
            return Ok(PseudoType::inferred(node, false));
        }
        Ok(PseudoType::Tuple(
            elements
                .into_iter()
                .map(|element| self.pseudo_expression_type(element))
                .collect::<Result<_, _>>()?,
        ))
    }
    fn pseudo_object(&mut self, node: N) -> Result<PseudoType, DemandError> {
        let members = self.pseudo_children(node, "properties")?;
        let mut errors = Vec::new();
        for &member in &members {
            // Source::syntax accepts only a successful ordinary parse: Syntax
            // failures become InputError before CapturedSyntax is published.
            // Thus native ThisNodeHasError is absent on this ordinary tree.
            // This invariant does not extend to private JSDoc recovery trees.
            if matches!(
                self.declaration_kind(member)?,
                K::ShorthandPropertyAssignment | K::SpreadAssignment
            ) {
                errors.push(member);
                continue;
            }
            let name = self.pseudo_required_child(member, "name")?;
            match self.declaration_kind(name)? {
                K::PrivateIdentifier => errors.push(member),
                K::ComputedPropertyName => {
                    let expression = self.pseudo_required_child(name, "expression")?;
                    if !self.pseudo_primitive_literal(expression, false)? {
                        errors.push(name);
                    }
                }
                _ => {}
            }
        }
        if !errors.is_empty() {
            return Ok(PseudoType::Inferred {
                expression: node,
                signature_return: false,
                errors,
            });
        }
        let mut result = Vec::new();
        for member in members {
            let name = self.pseudo_required_child(member, "name")?;
            let optional = self.pseudo_child(member, "questionToken")?.is_some();
            let kind = match self.declaration_kind(member)? {
                K::PropertyAssignment => {
                    let initializer = self.pseudo_required_child(member, "initializer")?;
                    PseudoMemberKind::Property {
                        readonly: false,
                        ty: self.pseudo_expression_type(initializer)?,
                    }
                }
                K::MethodDeclaration => {
                    // NewPseudoObjectMethod arguments are evaluated in this
                    // order, unlike typeFromFunctionLikeExpression.
                    let type_parameters = self.pseudo_children(member, "typeParameters")?;
                    let parameters = self.pseudo_parameters(member)?;
                    let returns = Box::new(self.pseudo_create_return(member)?);
                    PseudoMemberKind::Method(PseudoSignature {
                        declaration: member,
                        parameters,
                        type_parameters,
                        returns,
                    })
                }
                K::GetAccessor | K::SetAccessor => {
                    let pair = self.pseudo_accessors(member)?;
                    let both_annotations = match (pair.get, pair.set) {
                        (Some(get), Some(set)) => {
                            self.pseudo_accessor_annotation(get)?.is_some()
                                && self.pseudo_accessor_annotation(set)?.is_some()
                        }
                        _ => false,
                    };
                    if both_annotations {
                        if self.declaration_kind(member)? == K::GetAccessor {
                            PseudoMemberKind::Get {
                                declaration: member,
                                ty: self.pseudo_accessor_type(member)?,
                            }
                        } else {
                            PseudoMemberKind::Set {
                                declaration: member,
                                parameter: self
                                    .pseudo_parameters(member)?
                                    .into_iter()
                                    .next()
                                    .ok_or_else(|| {
                                        super::super::internal("annotated setter has no parameter")
                                    })?,
                            }
                        }
                    } else {
                        if member != pair.first {
                            continue;
                        }
                        PseudoMemberKind::Property {
                            readonly: self.declaration_kind(member)? == K::GetAccessor
                                && pair.second.is_none(),
                            ty: self.pseudo_accessor_type(member)?,
                        }
                    }
                }
                _ => continue,
            };
            result.push(PseudoMember {
                name,
                optional,
                kind,
            });
        }
        Ok(PseudoType::Object(result))
    }

    /// Pseudochecker's syntactic `isContextuallyTyped` is intentionally not the
    /// checker's getContextualType: it stops at any matching ancestor.
    pub(in crate::project::semantic) fn pseudo_contextually_typed(
        &mut self,
        node: N,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(node.file)?;
        let ast = &captured.ast;
        let mut parent = captured.parents[node.node.index()];
        while let Some(id) = parent {
            let data = ast.node(id);
            if matches!(
                data.syntax_kind,
                K::CallExpression | K::SatisfiesExpression | K::JsxElement | K::JsxExpression
            ) || matches!(
                data.syntax_kind,
                K::VariableDeclaration
                    | K::Parameter
                    | K::PropertyDeclaration
                    | K::PropertySignature
                    | K::AsExpression
                    | K::TypeAssertionExpression
            ) && syntax::id(data, "type").is_some()
                && !syntax::is_const_assertion(ast, id)
            {
                return Ok(true);
            }
            parent = captured.parents[id.index()];
        }
        Ok(false)
    }
    /// `pseudochecker.IsInConstContext` walks only its exact propagating kinds.
    pub(in crate::project::semantic) fn pseudo_is_in_const_context(
        &mut self,
        node: N,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(node.file)?;
        let ast = &captured.ast;
        let mut parent = captured.parents[node.node.index()];
        while let Some(id) = parent {
            let kind = ast.node(id).syntax_kind;
            if matches!(kind, K::AsExpression | K::TypeAssertionExpression)
                || !matches!(
                    kind,
                    K::ArrayLiteralExpression
                        | K::ObjectLiteralExpression
                        | K::ParenthesizedExpression
                        | K::SpreadElement
                        | K::PropertyAssignment
                        | K::ShorthandPropertyAssignment
                        | K::TemplateSpan
                        | K::PrefixUnaryExpression
                )
            {
                return Ok(syntax::is_const_assertion(ast, id));
            }
            parent = captured.parents[id.index()];
        }
        Ok(false)
    }
    pub(in crate::project::semantic) fn pseudo_could_refer_to_undefined(
        &mut self,
        ty: &PseudoType,
    ) -> Result<bool, DemandError> {
        let _depth = self
            .control
            .as_ref()
            .map(|control| control.enter())
            .transpose()
            .map_err(super::super::resource_failure)?;
        Ok(match ty {
            PseudoType::NoResult(_)
            | PseudoType::Inferred { .. }
            | PseudoType::Atom(K::UndefinedKeyword) => true,
            PseudoType::MaybeConst {
                constant, regular, ..
            } => {
                if pseudo_is_undefined(constant) {
                    true
                } else {
                    self.pseudo_could_refer_to_undefined(regular)?
                }
            }
            PseudoType::Direct(node) => self.pseudo_type_node_could_refer_to_undefined(*node)?,
            PseudoType::Union(types) => {
                let mut any = false;
                for ty in types {
                    if self.pseudo_could_refer_to_undefined(ty)? {
                        any = true;
                        break;
                    }
                }
                any
            }
            _ => false,
        })
    }
    fn pseudo_type_node_could_refer_to_undefined(
        &mut self,
        mut node: N,
    ) -> Result<bool, DemandError> {
        let _depth = self
            .control
            .as_ref()
            .map(|control| control.enter())
            .transpose()
            .map_err(super::super::resource_failure)?;
        while self.declaration_kind(node)? == K::ParenthesizedType {
            node = self.pseudo_required_child(node, "type")?;
        }
        Ok(match self.declaration_kind(node)? {
            K::TypeReference
            | K::IndexedAccessType
            | K::TypeQuery
            | K::OptionalType
            | K::RestType
            | K::ImportType
            | K::ConditionalType
            | K::TypeOperator
            | K::TypePredicate
            | K::UndefinedKeyword => true,
            K::UnionType | K::IntersectionType => {
                let mut any = false;
                for child in self.pseudo_children(node, "types")? {
                    if self.pseudo_type_node_could_refer_to_undefined(child)? {
                        any = true;
                        break;
                    }
                }
                any
            }
            _ => false,
        })
    }
    fn pseudo_add_undefined(&mut self, ty: PseudoType) -> Result<PseudoType, DemandError> {
        if self.pseudo_could_refer_to_undefined(&ty)? {
            Ok(ty)
        } else {
            Ok(PseudoType::Union(vec![
                ty,
                PseudoType::Atom(K::UndefinedKeyword),
            ]))
        }
    }

    pub(in crate::project::semantic) fn pseudo_is_this_name(
        &mut self,
        node: N,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(node.file)?;
        Ok(syntax::is_this_identifier(&captured.ast, node.node))
    }
    pub(in crate::project::semantic) fn pseudo_predicate(
        &mut self,
        node: N,
    ) -> Result<Option<PseudoPredicate>, DemandError> {
        if self.declaration_kind(node)? != K::TypePredicate {
            return Ok(None);
        }
        let name = self.pseudo_required_child(node, "parameterName")?;
        let this_ = self.declaration_kind(name)? == K::ThisType;
        let name_text = if this_ {
            None
        } else {
            Some(self.pseudo_exact_name(name)?.into())
        };
        Ok(Some(PseudoPredicate {
            asserts: self.pseudo_child(node, "assertsModifier")?.is_some(),
            this_,
            name: name_text,
            ty: self.pseudo_child(node, "type")?,
        }))
    }
    pub(in crate::project::semantic) fn pseudo_exact_name(
        &mut self,
        node: N,
    ) -> Result<String, DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(node.file)?;
        Ok(
            super::reference_imports::exact_text(inputs, node.file, &captured.ast, node.node)?
                .to_owned(),
        )
    }

    pub(in crate::project::semantic) fn pseudo_part_of_parameter(
        &mut self,
        node: N,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(node.file)?;
        let mut current = node.node;
        while matches!(
            captured.ast.node(current).syntax_kind,
            K::BindingElement | K::ArrayBindingPattern | K::ObjectBindingPattern
        ) {
            let Some(parent) = captured.parents[current.index()] else {
                return Ok(false);
            };
            current = parent;
        }
        Ok(captured.ast.node(current).syntax_kind == K::Parameter)
    }
    pub(in crate::project::semantic) fn pseudo_is_type_node(
        &mut self,
        node: N,
    ) -> Result<bool, DemandError> {
        Ok(syntax::is_type_node_kind(self.declaration_kind(node)?))
    }
    pub(in crate::project::semantic) fn pseudo_is_declaration(
        &mut self,
        node: N,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(node.file)?;
        Ok(syntax::is_declaration(
            &captured.ast,
            |id| captured.parents[id.index()],
            node.node,
        ))
    }
    pub(in crate::project::semantic) fn pseudo_skip_parentheses(
        &mut self,
        node: N,
    ) -> Result<N, DemandError> {
        let captured = self.syntax(node.file)?;
        Ok(N {
            file: node.file,
            node: syntax::skip_parentheses(&captured.ast, node.node),
        })
    }
    pub(in crate::project::semantic) fn pseudo_single_quote(
        &mut self,
        node: N,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(node.file)?;
        let start = captured.ast.token_span(node.node).start;
        Ok(captured.ast.source().as_bytes().get(start) == Some(&b'\''))
    }
    pub(in crate::project::semantic) fn pseudo_source_text(
        &mut self,
        node: N,
    ) -> Result<String, DemandError> {
        let captured = self.syntax(node.file)?;
        let span = captured.ast.token_span(node.node);
        Ok(captured.ast.source()[span.start..span.end].to_owned())
    }
    pub(in crate::project::semantic) fn pseudo_node_length(
        &mut self,
        node: N,
    ) -> Result<usize, DemandError> {
        let captured = self.syntax(node.file)?;
        let span = captured.ast.node(node.node).span;
        Ok(span.end - span.start)
    }
    pub(in crate::project::semantic) fn pseudo_operator(
        &mut self,
        node: N,
    ) -> Result<u32, DemandError> {
        syntax::number(self.syntax(node.file)?.ast.node(node.node), "operator").ok_or_else(|| {
            super::super::internal("operator syntax lacks its required token kind").into()
        })
    }
    pub(in crate::project::semantic) fn pseudo_containing_function(
        &mut self,
        node: N,
    ) -> Result<Option<N>, DemandError> {
        let mut parent = self.declaration_parent(node)?;
        while let Some(node) = parent {
            if syntax::is_function_like_kind(self.declaration_kind(node)?) {
                return Ok(Some(node));
            }
            parent = self.declaration_parent(node)?;
        }
        Ok(None)
    }

    // Captured-node projection helpers own no checker policy.
    pub(in crate::project::semantic) fn pseudo_node_expression(
        &mut self,
        node: N,
    ) -> Result<ExpressionRoot, DemandError> {
        Ok(ExpressionRoot {
            file: node.file,
            site: crate::project::binding::flow::Site::new(&self.syntax(node.file)?.ast, node.node),
        })
    }
    pub(in crate::project::semantic) fn pseudo_annotation(
        &mut self,
        node: N,
    ) -> Result<TypeExpression, DemandError> {
        let inputs = self.inputs;
        let captured = self.syntax(node.file)?;
        Ok(super::type_expression(
            inputs,
            node.file,
            captured,
            &captured.parents,
            node.node,
        ))
    }
    pub(in crate::project::semantic) fn pseudo_child(
        &mut self,
        node: N,
        field: &str,
    ) -> Result<Option<N>, DemandError> {
        Ok(
            syntax::id(self.syntax(node.file)?.ast.node(node.node), field)
                .map(|id| N::new(node.file, id)),
        )
    }
    fn pseudo_required_child(&mut self, node: N, field: &str) -> Result<N, DemandError> {
        self.pseudo_child(node, field)?
            .ok_or_else(|| super::super::internal("pseudo syntax lost required child").into())
    }
    pub(in crate::project::semantic) fn pseudo_children(
        &mut self,
        node: N,
        field: &str,
    ) -> Result<Vec<N>, DemandError> {
        Ok(
            syntax::ids(self.syntax(node.file)?.ast.node(node.node), field)
                .iter()
                .map(|&id| N::new(node.file, id))
                .collect(),
        )
    }
    fn pseudo_raw_declarations(&mut self, node: N) -> Result<Vec<N>, DemandError> {
        let origin: ReadOrigin = self
            .raw_node_symbol_origin(node)?
            .ok_or_else(|| super::super::internal("native pseudo declaration has no raw symbol"))?;
        let consumer = self.declaration_location(node)?;
        let declarations = self
            .bindings()
            .reference_symbol_declarations(ReferenceSymbol::Known(origin))
            .map_err(|error| super::super::binding_failure_at(self.inputs, &consumer, error))?;
        declarations
            .into_iter()
            .map(|declaration| self.reference_declaration_identity(declaration))
            .collect()
    }
    fn pseudo_primitive_literal(
        &mut self,
        node: N,
        include_bigint: bool,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(node.file)?;
        Ok(syntax::is_primitive_literal_value(
            &captured.ast,
            node.node,
            include_bigint,
        ))
    }
}

fn pseudo_is_undefined(ty: &PseudoType) -> bool {
    matches!(ty, PseudoType::Atom(K::UndefinedKeyword))
        || matches!(ty, PseudoType::MaybeConst { constant, .. } if pseudo_is_undefined(constant))
}

pub(in crate::project::semantic) struct PseudoPredicate {
    pub(in crate::project::semantic) asserts: bool,
    pub(in crate::project::semantic) this_: bool,
    pub(in crate::project::semantic) name: Option<std::sync::Arc<str>>,
    pub(in crate::project::semantic) ty: Option<N>,
}

impl Source<'_, '_> {
    /// `getMeaningOfEntityNameReference` asks the full name's parent before
    /// `GetFirstIdentifier` selects the raw lookup token.
    pub(in crate::project::semantic) fn pseudo_entity_name(
        &mut self,
        node: N,
    ) -> Result<(N, super::super::evaluation::SymbolMeaning, String), DemandError> {
        use super::super::evaluation::SymbolMeaning;
        let captured = self.syntax(node.file)?;
        let ast = &captured.ast;
        let parent = captured.parents[node.node.index()]
            .ok_or_else(|| super::super::internal("entity name lacks parent"))?;
        let kind = ast.node(node.node).syntax_kind;
        let p = ast.node(parent);
        let meaning = if matches!(
            p.syntax_kind,
            K::TypeQuery | K::ComputedPropertyName | K::BinaryExpression
        ) || p.syntax_kind == K::ExpressionWithTypeArguments
            && !syntax::is_part_of_type_node(ast, &captured.parents, parent)
            || p.syntax_kind == K::TypePredicate
                && syntax::id(p, "parameterName") == Some(node.node)
        {
            SymbolMeaning::ValueExport
        } else if matches!(kind, K::QualifiedName | K::PropertyAccessExpression)
            || p.syntax_kind == K::ImportEqualsDeclaration
            || p.syntax_kind == K::QualifiedName && syntax::id(p, "left") == Some(node.node)
            || matches!(
                p.syntax_kind,
                K::PropertyAccessExpression | K::ElementAccessExpression
            ) && syntax::id(p, "expression") == Some(node.node)
        {
            SymbolMeaning::Namespace
        } else {
            SymbolMeaning::Type
        };
        let mut first = node;
        let mut suffix = Vec::new();
        loop {
            let kind = self.declaration_kind(first)?;
            let (left, right) = match kind {
                K::QualifiedName => ("left", "right"),
                K::PropertyAccessExpression => ("expression", "name"),
                _ => break,
            };
            let name = self.pseudo_required_child(first, right)?;
            suffix.push(self.pseudo_exact_name(name)?);
            first = self.pseudo_required_child(first, left)?;
        }
        suffix.reverse();
        let suffix = if suffix.is_empty() {
            String::new()
        } else {
            format!(".{}", suffix.join("."))
        };
        Ok((first, meaning, suffix))
    }
    pub(in crate::project::semantic) fn pseudo_declaration_name(
        &mut self,
        node: N,
    ) -> Result<bool, DemandError> {
        let captured = self.syntax(node.file)?;
        Ok(syntax::is_declaration_name(
            &captured.ast,
            |id| captured.parents[id.index()],
            node.node,
        ))
    }
}
