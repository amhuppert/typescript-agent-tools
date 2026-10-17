//! Native symbol names and declaration ordering, shared by checker consumers.

use super::collect::{Decl, DeclKind};
use super::ids::NativeHeaderId;
use super::member::{MemberKey, ReservedMember};
use super::semantic::{BoundSymbol, ReadOrigin, TypeBindingFailure};
use crate::Span;
use crate::project::sources::FileId;
use crate::resources::{Control, complete_or_abort};
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

mod strings;
use strings::NativeStringBuilder;
pub(in crate::project) use strings::{
    NativeByteInput, NativeStringError, NativeStringPart, NativeStringPosition,
    NativeStringSegment, NativeStringValue,
};

/// An actual node in the request's immutable captured tree. Equality uses
/// file and node identity, independently from compareNodes' span ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::project) struct NativeNodeIdentity {
    pub(in crate::project) file: FileId,
    pub(in crate::project) node: crate::NodeId,
}
// Exact file/node indices only; identity never compares source text or spans.

/// Exact native property-table identity. `getPropertyNameFromType` and
/// `getStringLiteralType` use the same canonical byte/identity components;
/// constructing or roundtripping a name never observes identity ordering.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(in crate::project) struct NativePropertyKey(NativeStringValue);

/// Established native name families. A transformed name may retain opaque
/// atoms without retaining a Private/Unique shape; its identity is still exact.
pub(in crate::project) enum NativePropertyKeyKind<'a> {
    Fixed(&'a [u8]),
    Private {
        class: NativeSymbolIdentity,
        description: &'a [u8],
    },
    Unique {
        symbol: NativeSymbolIdentity,
        underlying: NativePropertyKey,
    },
    Composed,
}

impl NativePropertyKey {
    /// Binder `GetSymbolNameForPrivateIdentifier`: the containing class's
    /// actual header identity and exact description form the native key.
    /// Construction does not observe the request's identity ordering.
    pub(in crate::project) fn private(
        class: NativeSymbolIdentity,
        description: &str,
        control: Option<&Control>,
    ) -> Self {
        let mut value = NativeStringBuilder::new(control);
        value.bytes(b"\xfe#");
        value.identity(class);
        value.bytes(b"@");
        value.bytes(description.as_bytes());
        Self(value.finish())
    }

    /// `getUniqueESSymbolType`: preserve the underlying name before the
    /// symbol's actual identity. Neither component is printed or ranked here.
    pub(in crate::project) fn unique(
        symbol: NativeSymbolIdentity,
        underlying: NativePropertyKey,
        control: Option<&Control>,
    ) -> Self {
        let mut value = NativeStringBuilder::new(control);
        value.bytes(b"\xfe@");
        value.value(&underlying.0);
        value.bytes(b"@");
        value.identity(symbol);
        Self(value.finish())
    }

    pub(in crate::project) fn kind(&self, control: Option<&Control>) -> NativePropertyKeyKind<'_> {
        if let Some(bytes) = self.0.fixed_part() {
            return NativePropertyKeyKind::Fixed(bytes);
        }
        if let Some((class, description)) = self.0.private_parts() {
            return NativePropertyKeyKind::Private { class, description };
        }
        if let Some((symbol, underlying)) = self.0.unique_parts(control) {
            return NativePropertyKeyKind::Unique {
                symbol,
                underlying: Self(underlying),
            };
        }
        NativePropertyKeyKind::Composed
    }

    pub(in crate::project) fn from_string_value(value: NativeStringValue) -> Self {
        Self(value)
    }
    pub(in crate::project) fn string_value(&self) -> &NativeStringValue {
        &self.0
    }

    /// Approximate display length under design B's generated-name rule:
    /// exact byte components and punctuation, one unit per opaque identity.
    /// Reading length never allocates or observes an ordering rank.
    pub(in crate::project) fn display_length(&self, control: Option<&Control>) -> usize {
        self.0.display_length(control)
    }

    pub(in crate::project) fn fixed(
        bytes: impl NativeByteInput,
        control: Option<&Control>,
    ) -> Self {
        Self(NativeStringValue::from_bytes(bytes, control))
    }

    /// Ordinary text and fixed internal names have exact bytes; generated
    /// identities deliberately have no serialized byte spelling.
    pub(in crate::project) fn fixed_bytes(&self) -> Option<&[u8]> {
        self.0.fixed_part()
    }

    /// `isReservedMemberName`: FE followed by a non-@, non-# byte. An opaque
    /// decimal identity following FE starts with a digit, also a reserved name.
    pub(in crate::project) fn is_reserved_member_name(&self) -> bool {
        let mut parts = self.0.parts();
        match parts.next() {
            Some(NativeStringPart::Bytes(bytes)) if bytes.first() == Some(&0xfe) => {
                if bytes.len() >= 2 {
                    bytes[1] != b'@' && bytes[1] != b'#'
                } else {
                    matches!(parts.next(), Some(NativeStringPart::Identity(_)))
                }
            }
            _ => false,
        }
    }

    /// `isHyphenatedJsxName` observes a literal '-' anywhere in Symbol.Name.
    /// Opaque GetSymbolId components contain positive decimal digits only.
    pub(in crate::project) fn is_hyphenated_jsx_name(&self) -> bool {
        self.0.parts().any(|part| match part {
            NativeStringPart::Bytes(bytes) => bytes.contains(&b'-'),
            NativeStringPart::Identity(_) => false,
        })
    }

    /// `IsKnownSymbol`: the native unique-name prefix denotes a symbol key.
    pub(in crate::project) fn is_known_symbol(&self) -> bool {
        self.0.byte_prefix(b"\xfe@")
    }
}

impl NativeNodeIdentity {
    pub(in crate::project) fn new(file: FileId, node: crate::NodeId) -> Self {
        Self { file, node }
    }
}

/// `Symbol.Name` is a Go byte string. Reserved names contain FE, which is not
/// UTF-8; neither display spelling nor a replacement character is its identity.
#[derive(Debug, Clone)]
pub(super) enum NativeSymbolName {
    Bytes(Arc<[u8]>),
    Generated(NativePropertyKey),
    Unavailable(&'static str),
}

/// `declareSymbolEx` observes its actual parent; `getDeclarationName` observes
/// the nearest containing class independently of that parent. Neither name
/// decision can be reconstructed from the declaration's export projection.
#[derive(Default)]
pub(super) struct NativeDeclarationNameContext {
    pub(super) parent: Option<NativeSymbolIdentity>,
    pub(super) private_class: Option<NativeSymbolIdentity>,
}

impl NativeSymbolName {
    /// Symbol.Name shares an established canonical fixed name or retains its
    /// opaque property key. No spelling, ordering demand or payload copy occurs.
    fn from_key(key: NativePropertyKey) -> Self {
        match key.0.shared_fixed_part() {
            Some(bytes) => Self::Bytes(bytes),
            None => Self::Generated(key),
        }
    }

    /// `getDeclarationName` for actual ambient ModuleDeclarations, including
    /// the internal global-augmentation name. Quotes are identity bytes.
    pub(super) fn module_declaration(name: &str, global: bool) -> Self {
        if global {
            Self::internal(b"global", None)
        } else {
            Self::Bytes(format!("\"{name}\"").into_bytes().into())
        }
    }
    fn internal(name: &[u8], _control: Option<&Control>) -> Self {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[0xfe]);
        bytes.extend_from_slice(name);
        Self::Bytes(std::sync::Arc::from(bytes))
    }

    /// `declareSymbolEx` / `bindAnonymousDeclaration`: capture the native
    /// declared symbol's name at the binding operation, separate from its
    /// lexical selection name. Reserved member keys already own their rule.
    pub(super) fn declaration(declaration: &Decl, context: NativeDeclarationNameContext) -> Self {
        // The exported default header is named before anonymous declaration
        // cases. Its paired local header keeps the written declaration name.
        if context.parent.is_some()
            && declaration
                .export
                .as_ref()
                .is_some_and(|(_, name)| name == "default")
        {
            return Self::Bytes(b"default".as_slice().into());
        }
        match declaration.kind {
            DeclKind::TypeLiteral => Self::internal(b"type", None),
            DeclKind::ObjectLiteral => Self::internal(b"object", None),
            DeclKind::Function if declaration.name.is_empty() => Self::internal(b"function", None),
            DeclKind::Class if declaration.name.is_empty() => Self::internal(b"class", None),
            _ => {
                if let Some(member) = &declaration.member {
                    return match &member.key {
                        MemberKey::Text(name) => Self::Bytes(name.as_bytes().into()),
                        MemberKey::Reserved(kind) => Self::internal(
                            match kind {
                                ReservedMember::Call => b"call",
                                ReservedMember::New => b"new",
                                ReservedMember::Index => b"index",
                                ReservedMember::Constructor => b"constructor",
                            },
                            None,
                        ),
                        MemberKey::Dynamic => Self::internal(b"computed", None),
                        MemberKey::Private(description) => match context.private_class {
                            Some(class) => Self::Generated(NativePropertyKey::private(
                                class,
                                description.as_str(),
                                None,
                            )),
                            // `getDeclarationName` uses the missing-name
                            // sentinel for a private name outside any class.
                            None => Self::internal(b"missing", None),
                        },
                        MemberKey::NonScalar(_) => Self::Unavailable(
                            "native symbol name contains an unrepresentable UTF-16 identity",
                        ),
                    };
                }
                let name = declaration.name.as_str();
                Self::Bytes(name.as_bytes().into())
            }
        }
    }

    pub(in crate::project) fn bytes(&self) -> Result<&[u8], TypeBindingFailure> {
        match self {
            Self::Bytes(bytes) => Ok(bytes),
            Self::Generated(_) => Err(TypeBindingFailure::Pending(
                "generated native names require the structured property-key operation",
            )),
            Self::Unavailable(message) => Err(TypeBindingFailure::Pending(message)),
        }
    }

    pub(super) fn key(&self) -> Result<NativePropertyKey, TypeBindingFailure> {
        match self {
            Self::Bytes(bytes) => Ok(NativePropertyKey::fixed(bytes.clone(), None)),
            Self::Generated(key) => Ok(key.clone()),
            Self::Unavailable(message) => Err(TypeBindingFailure::Pending(message)),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::project) struct NativeDeclaration {
    pub(in crate::project) file: FileId,
    pub(in crate::project) span: Span,
}

impl NativeDeclaration {
    /// `compareNodes`: a present actual node sorts before nil; accepted
    /// program file order precedes its native full-span Pos.
    pub(in crate::project) fn compare(left: Option<&Self>, right: Option<&Self>) -> Ordering {
        match (left, right) {
            (Some(left), Some(right)) => left
                .file
                .index()
                .cmp(&right.file.index())
                .then_with(|| left.span.start.cmp(&right.span.start)),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        }
    }
}

/// An immutable native symbol observation. Identity is equality-only: the
/// Store index is not ast.GetSymbolId's lazily allocated numeric order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::project) enum NativeSymbolIdentity {
    Bound(NativeHeaderId),
    Transient(usize),
    EmptyTypeLiteral,
}

// Every variant is a fixed-size identity/tag. Derived Eq/Hash observes no name,
// declaration payload or lazily allocated ordering rank.

#[derive(Debug, Clone)]
pub(in crate::project) struct NativeSymbol {
    identity: NativeSymbolIdentity,
    name: NativeSymbolName,
    // Copies of a header retain a latent failure without cloning its names.
    declaration: Result<Option<NativeDeclaration>, Arc<TypeBindingFailure>>,
}

impl NativeSymbol {
    /// `newSymbol` / `instantiateSymbol`: every new immutable header, including
    /// binder observations and declaration-only replacements, is created here.
    /// Name and latent-failure payloads are shared before publication. Captured
    /// identities retain no request control.
    fn publish(header: Self) -> Arc<Self> {
        std::sync::Arc::new(header)
    }

    fn retain_declaration(
        declaration: Result<Option<NativeDeclaration>, TypeBindingFailure>,
    ) -> Result<Option<NativeDeclaration>, Arc<TypeBindingFailure>> {
        declaration.map_err(std::sync::Arc::new)
    }

    /// `newSymbol`-based semantic factories retain their actual fresh identity,
    /// native name and first declaration without inventing a bound symbol.
    pub(in crate::project) fn created(
        identity: NativeSymbolIdentity,
        name: impl NativeByteInput,
        declaration: Option<NativeDeclaration>,
        control: Option<&Control>,
    ) -> Arc<Self> {
        Self::created_with_key(
            identity,
            NativePropertyKey::fixed(name, control),
            declaration,
        )
    }

    /// `Symbol.Declarations` pointer publication updates only latent first-node
    /// ordering. It neither observes Symbol.Name nor consumes unavailable facts.
    pub(in crate::project) fn with_declaration(
        &self,
        declaration: Result<Option<NativeDeclaration>, TypeBindingFailure>,
    ) -> Arc<Self> {
        Self::publish(Self {
            identity: self.identity,
            name: self.name.clone(),
            declaration: Self::retain_declaration(declaration),
        })
    }

    pub(in crate::project) fn created_with_key(
        identity: NativeSymbolIdentity,
        key: NativePropertyKey,
        declaration: Option<NativeDeclaration>,
    ) -> Arc<Self> {
        Self::publish(Self {
            identity,
            name: NativeSymbolName::from_key(key),
            declaration: Ok(declaration),
        })
    }

    /// initializeTypeCaches creates this real symbol with no declarations.
    pub(in crate::project) fn empty_type_literal(control: Option<&Control>) -> Arc<Self> {
        Self::publish(Self {
            identity: NativeSymbolIdentity::EmptyTypeLiteral,
            name: NativeSymbolName::internal(b"type", control),
            declaration: Ok(None),
        })
    }

    pub(in crate::project) fn same_symbol(&self, other: &Self) -> bool {
        self.identity == other.identity
    }

    pub(in crate::project) fn identity(&self) -> NativeSymbolIdentity {
        self.identity
    }

    /// `instantiateSymbol` copies the declaration/name header while creating
    /// a different actual symbol. Allocation identity is not ordering rank.
    pub(in crate::project) fn with_identity(&self, identity: NativeSymbolIdentity) -> Arc<Self> {
        Self::publish(Self {
            identity,
            ..self.clone()
        })
    }

    /// `checkObjectLiteral` creates a fresh named header while copying the
    /// original declaration observation without demanding it for ordering.
    pub(in crate::project) fn with_identity_and_key(
        &self,
        identity: NativeSymbolIdentity,
        key: NativePropertyKey,
    ) -> Arc<Self> {
        Self::publish(Self {
            identity,
            name: NativeSymbolName::from_key(key),
            declaration: self.declaration.clone(),
        })
    }

    /// `Symbol.Name` as an owned exact byte observation. The header keeps its
    /// shared payload; consumers needing ownership copy only through this owner.
    /// Observe an unavailable/generated name before copying its payload.
    pub(in crate::project) fn name(
        &self,
        _control: Option<&Control>,
    ) -> Result<Vec<u8>, TypeBindingFailure> {
        let bytes = self.name.bytes()?;

        Ok(bytes.to_vec())
    }

    pub(in crate::project) fn key(&self) -> Result<NativePropertyKey, TypeBindingFailure> {
        self.name.key()
    }
}

/// Request-local identity observations, shared by semantic key writes and
/// `compareSymbolsWorker`. Opaque identities keep actual symbol equality;
/// only an ordering demand assigns a rank. Presentation bookkeeping and
/// immutable binder allocation do not seed this order (the documented B rule).
#[derive(Default)]
pub(in crate::project) struct IdentityOrder {
    symbols: RefCell<HashMap<NativeSymbolIdentity, usize>>,
}

impl IdentityOrder {
    /// `compareSymbolsWorker` compares exact names. Generated-name ordering
    /// follows the adopted identity-component policy: unique underlying names
    /// compare before their symbol IDs, even when flattened bytes would choose
    /// a different order. New composed families require an explicit policy.
    pub(in crate::project) fn compare_property_keys(
        &self,
        left: &NativePropertyKey,
        right: &NativePropertyKey,
        control: Option<&Control>,
    ) -> Result<Ordering, TypeBindingFailure> {
        let _depth = control.map(|control| complete_or_abort(control.enter()));
        use NativePropertyKeyKind::*;
        Ok(match (left.kind(control), right.kind(control)) {
            (Fixed(a), Fixed(b)) => a.cmp(b),
            (
                Private {
                    class: a,
                    description: an,
                },
                Private {
                    class: b,
                    description: bn,
                },
            ) => self.compare(a, b, control).then_with(|| an.cmp(bn)),
            (
                Unique {
                    symbol: a,
                    underlying: an,
                },
                Unique {
                    symbol: b,
                    underlying: bn,
                },
            ) => self
                .compare_property_keys(&an, &bn, control)?
                .then_with(|| self.compare(a, b, control)),
            (Private { .. }, Unique { .. }) => Ordering::Less,
            (Unique { .. }, Private { .. }) => Ordering::Greater,
            (generated @ (Private { .. } | Unique { .. }), Fixed(fixed)) => {
                let prefix: &[u8] = if matches!(generated, Private { .. }) {
                    b"\xfe#"
                } else {
                    b"\xfe@"
                };
                let common = fixed.len().min(prefix.len());
                let leading = prefix[..common].cmp(&fixed[..common]);
                if leading != Ordering::Equal {
                    return Ok(leading);
                }
                if fixed.len() <= prefix.len() {
                    return Ok(Ordering::Greater);
                }
                match generated {
                    Unique { underlying, .. } => {
                        let tail = NativePropertyKey::fixed(&fixed[prefix.len()..], control);
                        self.compare_property_keys(&underlying, &tail, control)?
                            .then(Ordering::Greater)
                    }
                    // Fixed FE# names are a separate reserved family. They do
                    // not encode a class identity; generated private keys do.
                    Private { .. } => Ordering::Greater,
                    Fixed(_) | Composed => unreachable!("generated family matched above"),
                }
            }
            (Fixed(_), Private { .. } | Unique { .. }) => {
                self.compare_property_keys(right, left, control)?.reverse()
            }
            (Composed, _) | (_, Composed) if left.0.canonical_equal(&right.0) => Ordering::Equal,
            (Composed, _) | (_, Composed) => {
                return Err(TypeBindingFailure::Pending(
                    "native composed string ordering requires an opaque identity-component policy",
                ));
            }
        })
    }

    pub(in crate::project) fn compare_names(
        &self,
        left: &NativeSymbol,
        right: &NativeSymbol,
        control: Option<&Control>,
    ) -> Result<Ordering, TypeBindingFailure> {
        self.compare_property_keys(&left.key()?, &right.key()?, control)
    }

    pub(in crate::project) fn observe(
        &self,
        identity: NativeSymbolIdentity,
        _control: Option<&Control>,
    ) {
        self.rank(identity);
    }

    /// ast.GetSymbolId's one lazy request-local publication owner. Observation
    /// on a hit is constant work; only a new actual identity grows the table.
    fn rank(&self, identity: NativeSymbolIdentity) -> usize {
        let mut symbols = self.symbols.borrow_mut();
        if let Some(&rank) = symbols.get(&identity) {
            return rank;
        }
        symbols.reserve(1);
        let rank = symbols.len();
        symbols.insert(identity, rank);
        rank
    }

    fn compare(
        &self,
        left: NativeSymbolIdentity,
        right: NativeSymbolIdentity,
        _control: Option<&Control>,
    ) -> Ordering {
        let left = self.rank(left);
        let right = self.rank(right);
        left.cmp(&right)
    }

    /// `compareSymbolsWorker` and `compareNodes`. Actual declaration Pos and
    /// accepted program file order precede canonical byte names. The final tie
    /// observes left before right through this request's one lazy identity owner.
    pub(in crate::project) fn compare_symbols(
        &self,
        left: Option<&NativeSymbol>,
        right: Option<&NativeSymbol>,
        control: Option<&Control>,
    ) -> Result<Ordering, TypeBindingFailure> {
        let _depth = control.map(|control| complete_or_abort(control.enter()));
        let (left, right) = match (left, right) {
            (None, None) => return Ok(Ordering::Equal),
            (None, Some(_)) => return Ok(Ordering::Greater),
            (Some(_), None) => return Ok(Ordering::Less),
            (Some(left), Some(right)) => (left, right),
        };
        if left.same_symbol(right) {
            return Ok(Ordering::Equal);
        }
        let declarations = NativeDeclaration::compare(
            left.declaration
                .as_ref()
                .map_err(|failure| (**failure).clone())?
                .as_ref(),
            right
                .declaration
                .as_ref()
                .map_err(|failure| (**failure).clone())?
                .as_ref(),
        );
        if declarations != Ordering::Equal {
            return Ok(declarations);
        }
        let names = self.compare_names(left, right, control)?;
        if names != Ordering::Equal {
            return Ok(names);
        }
        Ok(self.compare(left.identity, right.identity, control))
    }
}

/// Tool-defined stable fallible merge-sort schedule for native symbol ordering.
/// Comparison failures cannot become Equal or another successful ordering.
/// Native identity-tie order deliberately uses this deterministic schedule,
/// rather than promising Go's process-global pdqsort comparison sequence.
pub(in crate::project) fn try_sort_by<T: Clone, E>(
    values: &mut [T],
    compare: &impl Fn(&T, &T) -> Result<Ordering, E>,
) -> Result<(), E> {
    let length = values.len();
    if length < 2 {
        return Ok(());
    }
    let mut width = 1usize;
    let mut merged = Vec::with_capacity(length);
    while width < length {
        merged.clear();
        let mut start = 0usize;
        while start < length {
            let middle = start.saturating_add(width).min(length);
            let end = middle.saturating_add(width).min(length);
            let (mut left, mut right) = (start, middle);
            while left < middle && right < end {
                if compare(&values[left], &values[right])? != Ordering::Greater {
                    merged.push(values[left].clone());
                    left += 1;
                } else {
                    merged.push(values[right].clone());
                    right += 1;
                }
            }
            merged.extend_from_slice(&values[left..middle]);
            merged.extend_from_slice(&values[right..end]);
            start = end;
        }
        values.clone_from_slice(&merged);
        width = width.saturating_mul(2);
    }
    Ok(())
}

impl super::NativeInputs<'_> {
    /// Raw native identity is independent of both declaration recipes and
    /// navigation's meaning projection. Explicit getMergedSymbol owns merging.
    pub(in crate::project) fn reference_symbol_identity(
        &self,
        origin: ReadOrigin,
    ) -> Result<NativeSymbolIdentity, TypeBindingFailure> {
        Ok(NativeSymbolIdentity::Bound(origin.header()))
    }

    /// Equality of the native symbol, independent of role-specific handles
    /// and of ast.GetSymbolId's separate lazy allocation ordering.
    pub(in crate::project) fn native_symbol_identity(
        &self,
        symbol: BoundSymbol,
    ) -> Result<NativeSymbolIdentity, TypeBindingFailure> {
        Ok(NativeSymbolIdentity::Bound(symbol.0))
    }

    /// `isDeclarationContainedBy` for an actual retained or synthetic-symbol
    /// ValueDeclaration span. Native intentionally does not compare files.
    pub(in crate::project) fn native_declaration_contained_by(
        &self,
        span: Span,
        container: BoundSymbol,
    ) -> Result<bool, TypeBindingFailure> {
        for own in self.native_declarations(container)? {
            let outer = self.files.file(own.file).decls[own.id.index()]
                .native_span
                .ok_or(TypeBindingFailure::Pending(
                    "native container declaration span is not retained",
                ))?;
            if span.start >= outer.start && span.end <= outer.end {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(in crate::project) fn native_symbol(
        &self,
        symbol: BoundSymbol,
    ) -> Result<Arc<NativeSymbol>, TypeBindingFailure> {
        let identity = self.native_symbol_identity(symbol)?;
        let name = self.native_symbol_name(symbol).clone();
        let declaration = self
            .reference_symbol_declarations(super::ReferenceSymbol::Known(ReadOrigin::Direct(
                symbol,
            )))
            .map(|declarations| {
                declarations.first().map(|own| NativeDeclaration {
                    file: own.file,
                    span: own.span,
                })
            });
        Ok(NativeSymbol::publish(NativeSymbol {
            identity,
            name,
            declaration: NativeSymbol::retain_declaration(declaration),
        }))
    }

    /// `Symbol.Name`: the original exact factory name is independent of the
    /// declaration observation used by `compareSymbolsWorker`.
    fn native_symbol_name(&self, symbol: BoundSymbol) -> &NativeSymbolName {
        self.raw
            .native
            .header(symbol.0)
            .name
            .as_ref()
            .unwrap_or(&NativeSymbolName::Unavailable(
                "native symbol's original factory name is not retained",
            ))
    }

    pub(super) fn native_symbol_key(
        &self,
        symbol: BoundSymbol,
    ) -> Result<NativePropertyKey, TypeBindingFailure> {
        self.native_symbol_name(symbol).key()
    }
}

#[cfg(test)]
mod identity_order_tests {
    use super::*;

    #[test]
    fn canonical_key_roundtrip_retains_identity_and_name_family() {
        let class = NativeSymbolIdentity::Transient(0);
        let symbol = NativeSymbolIdentity::Transient(1);
        for description in ["", "#field", "#\u{03c0}"] {
            let private = NativePropertyKey::private(class, description, None);
            let rebuilt = NativePropertyKey::from_string_value(NativeStringValue::from_parts(
                private.string_value().parts().flat_map(|part| match part {
                    NativeStringPart::Bytes(bytes) => bytes
                        .iter()
                        .map(|&byte| NativeStringPart::Bytes(vec![byte].into()))
                        .collect::<Vec<_>>(),
                    atom => vec![atom],
                }),
                None,
            ));
            assert_eq!(private, rebuilt);
            assert!(
                matches!(rebuilt.kind(None), NativePropertyKeyKind::Private { class: actual, description: actual_text }
                if actual == class && actual_text == description.as_bytes())
            );
            let unique = NativePropertyKey::unique(symbol, rebuilt.clone(), None);
            assert!(
                matches!(unique.kind(None), NativePropertyKeyKind::Unique { symbol: actual, underlying }
                if actual == symbol && underlying == rebuilt)
            );
            assert_ne!(
                private,
                NativePropertyKey::private(symbol, description, None)
            );
        }
        let exact = NativePropertyKey::fixed(b"\xfe#field".as_slice(), None);
        let replacement = NativePropertyKey::fixed("\u{fffd}#field".as_bytes(), None);
        assert_ne!(exact, replacement);
    }

    #[test]
    fn unique_key_prefix_and_separator_cannot_overlap() {
        let symbol = NativeSymbolIdentity::Transient(0);
        let composed = NativePropertyKey::from_string_value(NativeStringValue::from_parts(
            [
                NativeStringPart::Bytes(b"\xfe@".as_slice().into()),
                NativeStringPart::Identity(symbol),
            ],
            None,
        ));
        assert!(matches!(
            composed.kind(None),
            NativePropertyKeyKind::Composed
        ));
        for name in ["", "a"] {
            let underlying = NativePropertyKey::fixed(name.as_bytes(), None);
            let unique = NativePropertyKey::unique(symbol, underlying.clone(), None);
            assert!(
                matches!(unique.kind(None), NativePropertyKeyKind::Unique { symbol: actual, underlying: actual_name }
                if actual == symbol && actual_name == underlying)
            );
        }
    }

    #[test]
    fn unique_keys_compare_underlying_names_before_the_identity_separator() {
        let a = NativeSymbolIdentity::Transient(0);
        let b = NativeSymbolIdentity::Transient(1);
        let left =
            NativePropertyKey::unique(a, NativePropertyKey::fixed(b"a".as_slice(), None), None);
        let right =
            NativePropertyKey::unique(b, NativePropertyKey::fixed(b"a!".as_slice(), None), None);
        let order = IdentityOrder::default();
        // Comparing flattened bytes would put the '@' separator after '!'.
        // The adopted name-family policy compares the underlying names first.
        assert_eq!(
            order.compare_property_keys(&left, &right, None).unwrap(),
            Ordering::Less
        );
        // The decisive name comparison must not have observed either symbol.
        assert_eq!(order.compare(b, a, None), Ordering::Less);
        assert_eq!(
            order.compare_property_keys(&right, &left, None).unwrap(),
            Ordering::Greater
        );
    }

    #[test]
    fn composed_ordering_fails_without_observing_identity() {
        let a = NativeSymbolIdentity::Transient(0);
        let b = NativeSymbolIdentity::Transient(1);
        let composed = NativePropertyKey::from_string_value(NativeStringValue::from_parts(
            [
                NativeStringPart::Bytes(b"prefix".as_slice().into()),
                NativeStringPart::Identity(a),
            ],
            None,
        ));
        let order = IdentityOrder::default();
        assert!(matches!(
            order.compare_property_keys(
                &composed,
                &NativePropertyKey::fixed(b"other".as_slice(), None),
                None
            ),
            Err(TypeBindingFailure::Pending(_))
        ));
        assert_eq!(
            order
                .compare_property_keys(&composed, &composed, None)
                .unwrap(),
            Ordering::Equal
        );
        assert_eq!(order.compare(b, a, None), Ordering::Less);
    }

    fn header(id: usize, name: &[u8], declaration: Option<NativeDeclaration>) -> Arc<NativeSymbol> {
        NativeSymbol::created(NativeSymbolIdentity::Transient(id), name, declaration, None)
    }

    #[test]
    fn identity_ties_share_prior_observations_and_preserve_actual_equality() {
        let left = header(0, b"prototype", None);
        let right = header(1, b"prototype", None);
        for prior_right in [false, true] {
            let order = IdentityOrder::default();
            if prior_right {
                order.observe(right.identity(), None);
            }
            let expected = if prior_right {
                Ordering::Greater
            } else {
                Ordering::Less
            };
            assert_eq!(
                order
                    .compare_symbols(Some(&left), Some(&right), None)
                    .unwrap(),
                expected
            );
            assert_eq!(
                order
                    .compare_symbols(Some(&right), Some(&left), None)
                    .unwrap(),
                expected.reverse()
            );
            assert_eq!(
                order
                    .compare_symbols(Some(&left), Some(&left), None)
                    .unwrap(),
                Ordering::Equal
            );
        }
    }

    #[test]
    fn declaration_and_name_decisions_do_not_seed_identity_order() {
        let mut files = crate::project::sources::Sources::default();
        let first = files.reserve();
        let second = files.reserve();
        for (left, right) in [
            (header(0, b"a", None), header(1, b"b", None)),
            (
                header(
                    0,
                    b"z",
                    Some(NativeDeclaration {
                        file: first,
                        span: Span { start: 20, end: 30 },
                    }),
                ),
                header(
                    1,
                    b"a",
                    Some(NativeDeclaration {
                        file: second,
                        span: Span { start: 0, end: 10 },
                    }),
                ),
            ),
            (
                header(
                    0,
                    b"z",
                    Some(NativeDeclaration {
                        file: first,
                        span: Span { start: 0, end: 10 },
                    }),
                ),
                header(
                    1,
                    b"a",
                    Some(NativeDeclaration {
                        file: first,
                        span: Span { start: 20, end: 30 },
                    }),
                ),
            ),
        ] {
            let order = IdentityOrder::default();
            assert_eq!(
                order
                    .compare_symbols(Some(&left), Some(&right), None)
                    .unwrap(),
                Ordering::Less
            );
            // Fresh instantiated symbols retain each original header. A first
            // tie with the fresh symbol on the left must observe it first;
            // the earlier byte/node decision cannot have seeded either target.
            let left_clone = left.with_identity(NativeSymbolIdentity::Transient(2));
            let right_clone = right.with_identity(NativeSymbolIdentity::Transient(3));
            assert_eq!(
                order
                    .compare_symbols(Some(&left_clone), Some(&left), None)
                    .unwrap(),
                Ordering::Less
            );
            assert_eq!(
                order
                    .compare_symbols(Some(&right_clone), Some(&right), None)
                    .unwrap(),
                Ordering::Less
            );
        }
    }
}
