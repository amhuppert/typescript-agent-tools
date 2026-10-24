//! Actual target and ordered argument identities for native reference interning.
use super::keys::{KeyBuilder, same_types};
use super::{DemandError, Model, NativeCompletion, ObjectFlags, Type, TypeArgumentsState, TypeId};
use crate::resources::CheckedKey;
use std::sync::Arc;

/// `createTypeReferenceEx` / `getTypeListKey`: native storage is per target;
/// this request's shared table includes that target in the key. Object flags,
/// names and structural equality never participate in reference identity.
pub(super) struct ReferenceKey {
    target: TypeId,
    arguments: Arc<[TypeId]>,
    fingerprint: u64,
}

impl ReferenceKey {
    pub(super) fn new(target: TypeId, arguments: Arc<[TypeId]>) -> Self {
        let fingerprint = Self::hash(target, &arguments);
        Self {
            target,
            arguments,
            fingerprint,
        }
    }

    fn hash(target: TypeId, arguments: &[TypeId]) -> u64 {
        let mut hash = KeyBuilder::new();
        hash.fixed(&target);
        hash.types(arguments);
        hash.finish()
    }

    fn matches(&self, target: TypeId, arguments: &[TypeId]) -> bool {
        self.target == target && same_types(&self.arguments, arguments)
    }
}

impl CheckedKey for ReferenceKey {
    fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    fn equivalent(&self, other: &Self) -> bool {
        self.matches(other.target, &other.arguments)
    }
}

impl Model {
    /// `createTypeReference` has one identity cache: actual target plus the
    /// ordered actual argument identities. It never interns by names/origins.
    pub(in crate::project::semantic) fn reference(
        &mut self,
        target: TypeId,
        arguments: Vec<TypeId>,
    ) -> Result<TypeId, DemandError> {
        self.reference_ex(target, arguments, ObjectFlags::NONE)
    }

    /// `createTypeReferenceEx` checks the target/arguments cache before using
    /// its input flags, so FromTypeNode never upgrades an existing reference.
    pub(in crate::project::semantic) fn reference_ex(
        &mut self,
        target: TypeId,
        arguments: Vec<TypeId>,
        flags: ObjectFlags,
    ) -> Result<TypeId, DemandError> {
        let target_view = self.declared_target(target)?;
        let parameter_count =
            target_view.outer_parameters.len() + target_view.local_parameters.len();
        // `getTypeWithThisArgument` may supply the established synthetic this
        // slot. It is a distinct actual reference key, never an inferred slot.
        let with_this = target_view.this_parameter.is_some()
            && arguments.len() == target_view.all_parameters.len();
        if arguments.len() != parameter_count && !with_this {
            return Err(super::super::internal(
                "reference factory received unestablished argument arity",
            )
            .into());
        }
        let fingerprint = ReferenceKey::hash(target, &arguments);
        if let Some(&id) = self
            .references
            .get_by(fingerprint, |key| key.matches(target, &arguments))
        {
            return Ok(id);
        }
        let arguments = { std::sync::Arc::from(arguments) };
        let id = self.push_with_flags(
            Type::Reference {
                target,
                arguments: TypeArgumentsState::Installed(NativeCompletion::clean(Arc::clone(
                    &arguments,
                ))),
                deferred: None,
                alias: None,
                array_literal: false,
            },
            flags,
        );
        self.references.insert(
            ReferenceKey {
                target,
                arguments,
                fingerprint,
            },
            id,
        );
        Ok(id)
    }
}
