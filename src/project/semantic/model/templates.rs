//! Canonical template-literal headers; reduction belongs to evaluation.
use super::{AliasSymbol, LiteralPart, LiteralValue, Model, Type, TypeId, TypeSymbol};
use std::sync::Arc;

#[derive(Clone, PartialEq, Eq, Hash)]
pub(in crate::project::semantic) struct TemplateLiteralView {
    pub(in crate::project::semantic) texts: Arc<[LiteralValue]>,
    pub(in crate::project::semantic) types: Arc<[TypeId]>,
}
impl TemplateLiteralView {
    /// Native `slices.Equal(texts)` in `inferTypesFromTemplateLiteralType` and
    /// `structuredTypeRelatedToWorker` compares actual string values. Stop at
    /// the first unequal span; structural inequality alone cannot decide it.
    pub(in crate::project::semantic) fn semantic_texts_equal(
        &self,
        other: &Self,
        control: Option<&crate::resources::Control>,
    ) -> Result<bool, super::LiteralError> {
        if self.texts.len() != other.texts.len() {
            return Ok(false);
        }
        for (left, right) in self.texts.iter().zip(other.texts.iter()) {
            if !left.semantic_equal(right, control)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
/// `getTemplateTypeKey`: normalized text values and actual placeholder type
/// identities form the key. Hash collisions never replace exact comparison.
pub(super) struct TemplateLiteralKey {
    data: TemplateLiteralView,
    fingerprint: u64,
}
impl TemplateLiteralKey {
    fn new(data: TemplateLiteralView) -> Self {
        use std::hash::{DefaultHasher, Hash, Hasher};
        let mut hash = DefaultHasher::new();

        (data.texts.len(), data.types.len()).hash(&mut hash);
        for text in data.texts.iter() {
            text.fingerprint_into(&mut hash);
        }
        for ty in data.types.iter() {
            ty.hash(&mut hash);
        }
        Self {
            data,
            fingerprint: hash.finish(),
        }
    }
}
impl crate::resources::CheckedKey for TemplateLiteralKey {
    fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    fn equivalent(&self, other: &Self) -> bool {
        if self.data.texts.len() != other.data.texts.len()
            || self.data.types.len() != other.data.types.len()
        {
            return false;
        }
        for (left, right) in self.data.texts.iter().zip(other.data.texts.iter()) {
            if !left.canonical_equal(right) {
                return false;
            }
        }
        for (left, right) in self.data.types.iter().zip(other.data.types.iter()) {
            if left != right {
                return false;
            }
        }
        true
    }
}
impl Model {
    pub(in crate::project::semantic) fn numeric_string_type(&self) -> TypeId {
        self.numeric_string.expect("startup numeric string")
    }
    pub(in crate::project::semantic) fn template_constraint_type(&self) -> TypeId {
        self.template_constraint
            .expect("startup template constraint")
    }

    /// `newTemplateLiteralType` / `templateLiteralTypes`: normalized exact
    /// text segments and actual placeholder identities form the cache key.
    pub(in crate::project::semantic) fn template_literal(
        &mut self,
        texts: Vec<LiteralValue>,
        types: Vec<TypeId>,
    ) -> TypeId {
        assert_eq!(texts.len(), types.len() + 1);
        let data = TemplateLiteralView {
            texts: std::sync::Arc::from(texts),
            types: std::sync::Arc::from(types),
        };
        let key = TemplateLiteralKey::new(data);
        if let Some(&ty) = self.template_literals.get(&key) {
            return ty;
        }
        let ty = self.push(Type::TemplateLiteral(key.data.clone()));
        self.template_literals.insert(key, ty);
        ty
    }
}

/// Compiler-known intrinsic aliases are selected only after their declared
/// type is the actual IntrinsicMarker. Same-named ordinary aliases are inert.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::project::semantic) enum StringMappingKind {
    Uppercase,
    Lowercase,
    Capitalize,
    Uncapitalize,
}
impl StringMappingKind {
    pub(in crate::project::semantic) fn from_name(name: &[u8]) -> Option<Self> {
        Some(match name {
            b"Uppercase" => Self::Uppercase,
            b"Lowercase" => Self::Lowercase,
            b"Capitalize" => Self::Capitalize,
            b"Uncapitalize" => Self::Uncapitalize,
            _ => return None,
        })
    }
    pub(in crate::project::semantic) fn name(self) -> &'static str {
        match self {
            Self::Uppercase => "Uppercase",
            Self::Lowercase => "Lowercase",
            Self::Capitalize => "Capitalize",
            Self::Uncapitalize => "Uncapitalize",
        }
    }
    /// Native `applyStringMapping`: capitalizing transforms the first decoded
    /// rune and copies the remainder exactly. Full casing maps canonical byte
    /// runs independently: intervening positive-decimal atoms are uncased and
    /// non-Case_Ignorable, so they reset Final_Sigma context on both sides.
    pub(in crate::project::semantic) fn apply(
        self,
        text: &LiteralValue,
        control: Option<&crate::resources::Control>,
    ) -> LiteralValue {
        match self {
            Self::Uppercase | Self::Lowercase => LiteralValue::from_parts(
                text.parts().map(|part| match part {
                    LiteralPart::Bytes(bytes) => {
                        let mapped = if self == Self::Uppercase {
                            crate::strings::to_upper_js(&bytes)
                        } else {
                            crate::strings::to_lower_js(&bytes)
                        };
                        LiteralPart::Bytes(std::sync::Arc::from(mapped))
                    }
                    LiteralPart::Identity(identity) => LiteralPart::Identity(identity),
                }),
                control,
            ),
            Self::Capitalize | Self::Uncapitalize => {
                let mut parts = text.parts();
                let Some(LiteralPart::Bytes(bytes)) = parts.next() else {
                    // Empty values and an initial digit are unchanged.
                    return text.clone();
                };
                let (_, split) = crate::strings::decode_js_string_rune(&bytes);
                let mut result = if self == Self::Capitalize {
                    crate::strings::to_upper_js(&bytes[..split])
                } else {
                    crate::strings::to_lower_js(&bytes[..split])
                };
                result.extend_from_slice(&bytes[split..]);
                LiteralValue::from_parts(
                    std::iter::once(LiteralPart::Bytes(std::sync::Arc::from(result))).chain(parts),
                    control,
                )
            }
        }
    }
}
#[derive(Clone)]
pub(in crate::project::semantic) struct StringMappingView {
    pub(in crate::project::semantic) symbol: AliasSymbol,
    pub(in crate::project::semantic) kind: StringMappingKind,
    pub(in crate::project::semantic) target: TypeId,
}
impl Model {
    pub(in crate::project::semantic) fn intrinsic_marker_type(&self) -> TypeId {
        self.intrinsic_marker.expect("startup intrinsic marker")
    }
    pub(in crate::project::semantic) fn is_intrinsic_marker(&self, ty: TypeId) -> bool {
        self.intrinsic_marker == Some(ty)
    }
    /// `getStringMappingTypeForGenericType` keys the actual alias symbol and
    /// target. Display spelling and structural equality are not identities.
    pub(in crate::project::semantic) fn string_mapping_type(
        &mut self,
        symbol: AliasSymbol,
        native_symbol: TypeSymbol,
        kind: StringMappingKind,
        target: TypeId,
    ) -> TypeId {
        if let Some(&ty) = self.string_mappings.get(&(symbol, target)) {
            return ty;
        }
        let ty = self.push(Type::StringMapping(StringMappingView {
            symbol,
            kind,
            target,
        }));
        self.symbols[ty.0] = native_symbol;
        self.string_mappings.insert((symbol, target), ty);
        ty
    }
}
