//! Conditional and substitution identities contain established headers only.
//! Evaluator owns the roots, lazy branches, inference and native cache slots.
use super::{Alias, MapperId, Model, Type, TypeFlags, TypeId};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::project::semantic) struct ConditionalRootId(pub(in crate::project::semantic) usize);

#[derive(Clone, Copy)]
pub(in crate::project::semantic) struct ConditionalView {
    pub(in crate::project::semantic) root: ConditionalRootId,
    pub(in crate::project::semantic) declaration: crate::project::binding::NativeDeclaration,
    pub(in crate::project::semantic) check_type: TypeId,
    pub(in crate::project::semantic) extends_type: TypeId,
    pub(in crate::project::semantic) mapper: Option<MapperId>,
    pub(in crate::project::semantic) combined_mapper: Option<MapperId>,
}

#[derive(Clone, Copy)]
pub(in crate::project::semantic) struct SubstitutionView {
    pub(in crate::project::semantic) base: TypeId,
    pub(in crate::project::semantic) constraint: TypeId,
}

impl Model {
    /// `isNoInferType` recognizes the actual substitution constraint flag.
    pub(in crate::project::semantic) fn is_no_infer_type(&self, ty: TypeId) -> bool {
        matches!(self.types.get(ty.0), Some(Type::Substitution(view)) if self.has_type_flags(view.constraint, TypeFlags::UNKNOWN))
    }
    /// `newConditionalType`: this identity is fresh; only the root's native
    /// instantiation cache may reuse it. Inputs have already been instantiated.
    pub(in crate::project::semantic) fn conditional_type(
        &mut self,
        data: ConditionalView,
        alias: Option<Alias>,
    ) -> TypeId {
        self.push(Type::Conditional {
            data,
            alias: alias.map(Arc::new),
        })
    }

    /// `getSubstitutionType` / `getOrCreateSubstitutionType` use one cache
    /// keyed by actual base and constraint identities, without flattening them.
    pub(in crate::project::semantic) fn substitution_type(
        &mut self,
        base: TypeId,
        constraint: TypeId,
    ) -> TypeId {
        if self.has_type_flags(constraint, TypeFlags::ANY | TypeFlags::UNKNOWN)
            || constraint == base
            || self.has_type_flags(base, TypeFlags::ANY)
        {
            return base;
        }
        self.substitution_type_unchecked(base, constraint)
    }

    pub(in crate::project::semantic) fn substitution_type_unchecked(
        &mut self,
        base: TypeId,
        constraint: TypeId,
    ) -> TypeId {
        if let Some(&ty) = self.substitutions.get(&(base, constraint)) {
            return ty;
        }
        let ty = self.push(Type::Substitution(SubstitutionView { base, constraint }));
        self.substitutions.insert((base, constraint), ty);
        ty
    }
}
