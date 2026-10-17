//! Output syntax for the native node builder. This contains no checker facts
//! and its printer performs no type, symbol, or declaration acquisition.

use crate::printer::{TypePosition as Position, TypePrecedence as Precedence};

#[derive(Clone)]
pub(super) enum Node {
    Text(String),
    Parenthesized(Box<Node>),
    Reference(String, Vec<Node>),
    Named(Box<Node>, Vec<Node>),
    TypeParameter(TypeParameter),
    ExpressionAccess {
        left: Box<Node>,
        name: Box<Node>,
        computed: bool,
    },
    Import {
        specifier: String,
        typeof_: bool,
        qualifier: Option<Box<Node>>,
        arguments: Vec<Node>,
        resolution_mode: Option<crate::project::binding::ResolutionMode>,
    },
    Prefix(&'static str, Box<Node>),
    Array(Box<Node>),
    Tuple(Vec<Element>),
    Union(Vec<Node>),
    Intersection(Vec<Node>),
    Indexed(Box<Node>, Box<Node>),
    Template(Vec<Vec<u8>>, Vec<Node>),
    ReusedTemplate(Vec<String>, Vec<Node>),
    Conditional(Box<[Node; 4]>),
    Infer(TypeParameter),
    Object(Vec<Member>),
    Mapped {
        readonly: &'static str,
        parameter: TypeParameter,
        name: Option<Box<Node>>,
        optional: &'static str,
        template: Option<Box<Node>>,
    },
    Function(Box<Signature>, bool),
    Predicate {
        asserts: bool,
        name: String,
        ty: Option<Box<Node>>,
    },
}

#[derive(Clone)]
pub(super) struct Element {
    pub(super) name: Option<String>,
    pub(super) optional: bool,
    pub(super) rest: bool,
    pub(super) ty: Node,
}
#[derive(Clone)]
pub(super) struct TypeParameter {
    pub(super) modifiers: String,
    pub(super) name: String,
    pub(super) constraint: Option<Box<Node>>,
    pub(super) default: Option<Box<Node>>,
}
#[derive(Clone)]
pub(super) struct Signature {
    pub(super) type_parameters: Vec<TypeParameter>,
    pub(super) parameters: Vec<Element>,
    pub(super) returns: Node,
    pub(super) abstract_: bool,
}
#[derive(Clone, Copy)]
pub(super) enum SignatureKind {
    Call,
    Construct,
    Method,
    Get,
    Set,
}
#[derive(Clone)]
pub(super) enum Member {
    Property {
        name: String,
        optional: bool,
        readonly: bool,
        ty: Node,
    },
    Signature {
        kind: SignatureKind,
        name: String,
        optional: bool,
        signature: Signature,
    },
    Index {
        name: String,
        readonly: bool,
        key: Node,
        value: Node,
    },
    Elision(String),
}

impl Node {
    /// `appendReferenceToType`: imported/reference roots shift arguments to
    /// the appended reference; instantiation-expression roots keep their own.
    pub(super) fn append_reference(
        self,
        name: Node,
        arguments: Vec<Node>,
        instantiation: bool,
    ) -> Self {
        fn append(left: Node, right: Node) -> Node {
            match right {
                Node::ExpressionAccess {
                    left: prefix,
                    name,
                    computed: false,
                } => append(append(left, *prefix), *name),
                right => Node::ExpressionAccess {
                    left: Box::new(left),
                    name: Box::new(right),
                    computed: false,
                },
            }
        }
        match self {
            Self::Reference(root, old_arguments) => Self::Named(
                Box::new(Self::text(root)),
                old_arguments,
            )
            .append_reference(name, arguments, instantiation),
            Self::Import {
                specifier,
                typeof_,
                qualifier,
                resolution_mode,
                ..
            } => Self::Import {
                specifier,
                typeof_,
                qualifier: Some(Box::new(match qualifier {
                    Some(left) => append(*left, name),
                    None => name,
                })),
                arguments,
                resolution_mode,
            },
            Self::Named(root, old_arguments) => {
                if instantiation && !old_arguments.is_empty() {
                    append(Self::Named(root, old_arguments), name)
                } else {
                    Self::Named(Box::new(append(*root, name)), arguments)
                }
            }
            root => append(root, name),
        }
    }
    pub(super) fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }
    pub(super) fn reference_name(&self) -> Option<&str> {
        match self {
            Self::Reference(name, _) if !name.contains('.') => Some(name),
            Self::Named(name, _) => match &**name {
                Self::Text(name) => Some(name),
                _ => None,
            },
            _ => None,
        }
    }
    /// TypeScript printer/parenthesizer precedence for emitted type syntax.
    fn precedence(&self) -> Precedence {
        match self {
            Self::Function(..) => Precedence::Function,
            Self::Conditional(_) => Precedence::Conditional,
            Self::Union(_) => Precedence::Union,
            Self::Intersection(_) => Precedence::Intersection,
            Self::Prefix(..) => Precedence::TypeOperator,
            Self::Infer(parameter) => {
                crate::printer::infer_type_precedence(parameter.constraint.is_some())
            }
            Self::Array(_) | Self::Indexed(..) => Precedence::Postfix,
            _ => Precedence::NonArray,
        }
    }
    pub(super) fn print(&self) -> Vec<u8> {
        let mut output = String::new();
        self.write(&mut output, Position::Outside, false);
        output.into_bytes()
    }
    fn write(&self, output: &mut String, position: Position, parent_in_extends: bool) {
        let state = position.state(parent_in_extends);
        let constrained_infer =
            matches!(self, Self::Infer(parameter) if parameter.constraint.is_some());
        let parens = state.parenthesizes(self.precedence(), constrained_infer);
        let in_extends = state.in_extends && !parens;
        if parens {
            output.push('(');
        }
        match self {
            Self::Text(text) => output.push_str(text),
            Self::Parenthesized(node) => {
                output.push('(');
                node.write(output, Position::Outside, false);
                output.push(')');
            }
            Self::Reference(name, args) => {
                output.push_str(name);
                arguments(output, args);
            }
            Self::Named(name, args) => {
                name.write(output, Position::Outside, false);
                arguments(output, args);
            }
            Self::TypeParameter(parameter) => parameter.write(output, false),
            Self::ExpressionAccess {
                left,
                name,
                computed,
            } => {
                left.write(output, Position::Outside, false);
                output.push(if *computed { '[' } else { '.' });
                name.write(output, Position::Outside, false);
                if *computed {
                    output.push(']');
                }
            }
            Self::Import {
                specifier,
                typeof_,
                qualifier,
                arguments: args,
                resolution_mode,
            } => {
                if *typeof_ {
                    output.push_str("typeof ");
                }
                output.push_str("import(");
                output.push_str(&crate::printer::quoted_text(specifier, '"'));
                if let Some(mode) = resolution_mode {
                    output.push_str(", { with: { \"resolution-mode\": \"");
                    output.push_str(match mode {
                        crate::project::binding::ResolutionMode::Import => "import",
                        crate::project::binding::ResolutionMode::Require => "require",
                    });
                    output.push_str("\" } }");
                }
                output.push(')');
                if let Some(qualifier) = qualifier {
                    output.push('.');
                    qualifier.write(output, Position::Outside, false);
                }
                arguments(output, args);
            }
            Self::Prefix(prefix, operand) => {
                output.push_str(prefix);
                operand.write(
                    output,
                    Position::Operand(if *prefix == "readonly " {
                        Precedence::Postfix
                    } else {
                        Precedence::TypeOperator
                    }),
                    in_extends,
                );
            }
            Self::Array(element) => {
                element.write(output, Position::Operand(Precedence::Postfix), in_extends);
                output.push_str("[]");
            }
            Self::Tuple(elements) => {
                output.push('[');
                for (i, element) in elements.iter().enumerate() {
                    if i != 0 {
                        output.push_str(", ");
                    }
                    element.write(output);
                }
                output.push(']');
            }
            Self::Union(types) | Self::Intersection(types) => {
                let separator = if matches!(self, Self::Union(_)) {
                    " | "
                } else {
                    " & "
                };
                for (i, ty) in types.iter().enumerate() {
                    if i != 0 {
                        output.push_str(separator);
                    }
                    ty.write(
                        output,
                        Position::Operand(Precedence::TypeOperator),
                        in_extends,
                    );
                }
            }
            Self::Indexed(object, index) => {
                object.write(output, Position::Operand(Precedence::Postfix), in_extends);
                output.push('[');
                index.write(output, Position::Outside, in_extends);
                output.push(']');
            }
            Self::Template(texts, types) => {
                output.push('`');
                for (i, text) in texts.iter().enumerate() {
                    let quoted = crate::printer::quoted_bytes(text, '`');
                    output.push_str(&quoted[1..quoted.len() - 1]);
                    if let Some(ty) = types.get(i) {
                        output.push_str("${");
                        ty.write(output, Position::Outside, in_extends);
                        output.push('}');
                    }
                }
                output.push('`');
            }
            Self::ReusedTemplate(tokens, types) => {
                for (i, token) in tokens.iter().enumerate() {
                    output.push_str(token);
                    if let Some(ty) = types.get(i) {
                        ty.write(output, Position::Outside, in_extends);
                    }
                }
            }
            Self::Conditional(parts) => {
                parts[0].write(output, Position::Operand(Precedence::Union), in_extends);
                output.push_str(" extends ");
                parts[1].write(output, Position::Extends, in_extends);
                output.push_str(" ? ");
                parts[2].write(output, Position::Outside, in_extends);
                output.push_str(" : ");
                parts[3].write(output, Position::Outside, in_extends);
            }
            Self::Infer(parameter) => {
                output.push_str("infer ");
                parameter.write(output, true);
            }
            Self::Object(members) => {
                if members.is_empty() {
                    output.push_str("{}");
                } else {
                    output.push_str("{ ");
                    for member in members {
                        member.write(output);
                        output.push_str("; ");
                    }
                    output.push('}');
                }
            }
            Self::Mapped {
                readonly,
                parameter,
                name,
                optional,
                template,
            } => {
                output.push_str("{ ");
                output.push_str(readonly);
                output.push('[');
                output.push_str(&parameter.name);
                output.push_str(" in ");
                if let Some(constraint) = &parameter.constraint {
                    constraint.write(output, Position::Outside, in_extends);
                }
                if let Some(name) = name {
                    output.push_str(" as ");
                    name.write(output, Position::Outside, in_extends);
                }
                output.push(']');
                output.push_str(optional);
                if let Some(template) = template {
                    output.push_str(": ");
                    template.write(output, Position::Outside, in_extends);
                }
                output.push_str("; }");
            }
            Self::Function(signature, construct) => {
                if *construct {
                    if signature.abstract_ {
                        output.push_str("abstract ");
                    }
                    output.push_str("new ");
                }
                signature.write(output, true, in_extends);
            }
            Self::Predicate { asserts, name, ty } => {
                if *asserts {
                    output.push_str("asserts ");
                }
                output.push_str(name);
                if let Some(ty) = ty {
                    output.push_str(" is ");
                    ty.write(output, Position::Outside, in_extends);
                }
            }
        }
        if parens {
            output.push(')');
        }
    }
}
fn arguments(output: &mut String, arguments: &[Node]) {
    if arguments.is_empty() {
        return;
    }
    output.push('<');
    for (i, argument) in arguments.iter().enumerate() {
        if i != 0 {
            output.push_str(", ");
        }
        argument.write(output, Position::Outside, false);
    }
    output.push('>');
}
impl Element {
    fn write(&self, output: &mut String) {
        if self.rest {
            output.push_str("...");
        }
        if let Some(name) = &self.name {
            output.push_str(name);
            if self.optional {
                output.push('?');
            }
            output.push_str(": ");
        }
        self.ty.write(
            output,
            if self.name.is_none() && self.optional {
                Position::OutsideAt(Precedence::Postfix)
            } else {
                Position::Outside
            },
            false,
        );
        if self.name.is_none() && self.optional {
            output.push('?');
        }
    }
}
impl TypeParameter {
    fn write(&self, output: &mut String, infer: bool) {
        output.push_str(&self.modifiers);
        output.push_str(&self.name);
        if let Some(constraint) = &self.constraint {
            output.push_str(" extends ");
            constraint.write(
                output,
                if infer {
                    Position::Extends
                } else {
                    Position::Outside
                },
                false,
            );
        }
        if !infer && let Some(default) = &self.default {
            output.push_str(" = ");
            default.write(output, Position::Outside, false);
        }
    }
}
impl Signature {
    fn write(&self, output: &mut String, arrow: bool, in_extends: bool) {
        if !self.type_parameters.is_empty() {
            output.push('<');
            for (i, parameter) in self.type_parameters.iter().enumerate() {
                if i != 0 {
                    output.push_str(", ");
                }
                parameter.write(output, false);
            }
            output.push('>');
        }
        output.push('(');
        for (i, parameter) in self.parameters.iter().enumerate() {
            if i != 0 {
                output.push_str(", ");
            }
            parameter.write(output);
        }
        output.push_str(if arrow { ") => " } else { "): " });
        self.returns.write(
            output,
            if arrow {
                Position::ArrowReturn
            } else {
                Position::Outside
            },
            in_extends,
        );
    }
    pub(super) fn print(&self, construct: bool) -> Vec<u8> {
        let mut output = String::new();
        if construct {
            output.push_str("new ");
        }
        self.write(&mut output, false, false);
        output.into_bytes()
    }
}
impl Member {
    fn write(&self, output: &mut String) {
        match self {
            Self::Property {
                name,
                optional,
                readonly,
                ty,
            } => {
                if *readonly {
                    output.push_str("readonly ");
                }
                output.push_str(name);
                if *optional {
                    output.push('?');
                }
                output.push_str(": ");
                ty.write(output, Position::Outside, false);
            }
            Self::Index {
                name,
                readonly,
                key,
                value,
            } => {
                if *readonly {
                    output.push_str("readonly ");
                }
                output.push('[');
                output.push_str(name);
                output.push_str(": ");
                key.write(output, Position::Outside, false);
                output.push_str("]: ");
                value.write(output, Position::Outside, false);
            }
            Self::Signature {
                kind,
                name,
                optional,
                signature,
            } => {
                match kind {
                    SignatureKind::Construct => output.push_str("new "),
                    SignatureKind::Get => output.push_str("get "),
                    SignatureKind::Set => output.push_str("set "),
                    _ => {}
                }
                output.push_str(name);
                if *optional {
                    output.push('?');
                }
                if matches!(kind, SignatureKind::Set) {
                    output.push('(');
                    for (i, parameter) in signature.parameters.iter().enumerate() {
                        if i != 0 {
                            output.push_str(", ");
                        }
                        parameter.write(output);
                    }
                    output.push(')');
                } else {
                    signature.write(output, false, false);
                }
            }
            Self::Elision(text) => output.push_str(text),
        }
    }
}
