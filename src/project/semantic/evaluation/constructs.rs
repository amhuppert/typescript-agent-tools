//! Q5's construction question for one object literal (docs/constructs-design.md):
//! which object alternatives of a declared target its contextual type
//! constructs, and which of their properties the literal does not certainly
//! provide in the compiler's named-property model. Selection, enumeration and
//! result assembly belong to `semantic::constructs`.

use super::super::model::{DemandError, ObjectFlags, TypeFlags, TypeId};
use super::super::source::ExpressionRoot;
use super::super::source::object_literals::{ObjectLiteralRecipe, ObjectMember};
use super::contextual::{ContextFlags, ContextNode};
use super::{Evaluator, ReferenceSymbol};
use crate::constructs::{LiteralUniverse, Presence};
use crate::project::binding::NativePropertyKey;
use crate::references::Location;
use crate::syntax_kind::SyntaxKind as K;
use crate::types::TypeKey;

/// A declared target's object alternatives, whether instances of a generic
/// declaration are recognized through instantiation provenance, and the keys
/// its alternatives declare: None when an alternative admits every key
/// (`admits_every_key`).
pub(in crate::project::semantic) struct ConstructionTarget {
    pub(in crate::project::semantic) alternatives: Vec<TypeId>,
    generic: bool,
    keys: Option<Vec<NativePropertyKey>>,
}

impl ConstructionTarget {
    /// The literals a search examines: every one when requested or when any
    /// key may be a target key, otherwise those `in_default_universe` admits.
    pub(in crate::project::semantic) fn universe(&self, all_literals: bool) -> LiteralUniverse {
        if all_literals || self.keys.is_none() {
            LiteralUniverse::All
        } else {
            LiteralUniverse::TargetKeys
        }
    }
}

pub(in crate::project::semantic) enum TargetForm {
    Supported(ConstructionTarget),
    /// Why construction sites of this declaration cannot be identified.
    Refused(&'static str),
}

/// One literal's constructed alternatives, with `contextual` its contextual
/// type as discrimination leaves it.
pub(in crate::project::semantic) struct ConstructionFacts {
    pub(in crate::project::semantic) contextual: TypeId,
    pub(in crate::project::semantic) constructs: Vec<ConstructedAlternative>,
}

/// `instance` is the alternative as the literal's context instantiates it;
/// `alternative` indexes `ConstructionTarget::alternatives`.
pub(in crate::project::semantic) struct ConstructedAlternative {
    pub(in crate::project::semantic) alternative: usize,
    pub(in crate::project::semantic) instance: TypeId,
    pub(in crate::project::semantic) lacks: Vec<LackFact>,
}

pub(in crate::project::semantic) struct LackFact {
    pub(in crate::project::semantic) key: TypeKey,
    pub(in crate::project::semantic) optional: bool,
    pub(in crate::project::semantic) presence: Presence,
}

/// What a literal is known to provide: its written keys, when it has no
/// spread and no late-bound name, or else its checked type.
enum Provided {
    Written(Vec<NativePropertyKey>),
    Checked(TypeId),
}

impl Evaluator<'_, '_> {
    /// The declared type's object alternatives: its union constituents (or
    /// itself) that are object types or intersections of them, without call
    /// or construct signatures, which no object literal's type has. A declaration
    /// whose alternative's properties depend on type arguments
    /// (`isGenericObjectType`), or whose generic alternative keeps no
    /// provenance on its instantiations, is refused rather than matched
    /// partially.
    pub(in crate::project::semantic) fn construction_target(
        &mut self,
        symbol: ReferenceSymbol,
        declared: TypeId,
        consumer: &Location,
    ) -> Result<TargetForm, DemandError> {
        let constituents = match self.model.union_constituents(declared) {
            Some(types) => types.to_vec(),
            None => vec![declared],
        };
        let mut alternatives = Vec::new();
        for ty in constituents {
            if self.object_like(ty)
                && self.signatures_of_type(ty, false, consumer)?.is_empty()
                && self.signatures_of_type(ty, true, consumer)?.is_empty()
            {
                alternatives.push(ty);
            }
        }
        if alternatives.is_empty() {
            return Ok(TargetForm::Refused(
                "its declared type has no object type an object literal can construct",
            ));
        }
        let generic = !self
            .local_type_parameters_of_symbol_completion(symbol, consumer)?
            .admit()?
            .is_empty();
        let mut keys = Some(Vec::new());
        for &alternative in &alternatives {
            if self.is_generic_object_type(alternative, consumer)? {
                return Ok(TargetForm::Refused(
                    "its properties depend on its type arguments",
                ));
            }
            if generic && !self.instance_provenance(alternative, consumer)? {
                return Ok(TargetForm::Refused(
                    "the instantiations of one of its alternatives keep no identity to recognize",
                ));
            }
            if self.admits_every_key(alternative, consumer)? {
                keys = None;
            }
            if let Some(keys) = &mut keys {
                for property in self
                    .properties_of_type_completion(alternative, consumer)?
                    .admit()?
                {
                    let symbol = self.property_symbol(&property, consumer)?;
                    let key = self.property_lookup_key(symbol, consumer)?;
                    if !keys.contains(&key) {
                        keys.push(key);
                    }
                }
            }
        }
        Ok(TargetForm::Supported(ConstructionTarget {
            alternatives,
            generic,
            keys,
        }))
    }

    /// Whether writing any key constructs this alternative without an
    /// excess-property error: an index signature can address it, or
    /// `hasExcessProperties` skips the target, as it does an empty object
    /// type and the global Object type. A literal naming only other keys can
    /// then construct it validly.
    fn admits_every_key(
        &mut self,
        alternative: TypeId,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        Ok(!self.index_infos_of_type(alternative, consumer)?.is_empty()
            || self.is_empty_object_type(alternative, consumer)?
            || alternative == self.global_object_type("Object", consumer)?)
    }

    /// The string keys a target alternative declares, for `--missing`.
    pub(in crate::project::semantic) fn construction_keys(
        &mut self,
        alternative: TypeId,
        consumer: &Location,
    ) -> Result<Vec<String>, DemandError> {
        let mut keys = Vec::new();
        for property in self
            .properties_of_type_completion(alternative, consumer)?
            .admit()?
        {
            if let TypeKey::String { name } = property.key_snapshot() {
                keys.push(name);
            }
        }
        Ok(keys)
    }

    /// An object type, or an intersection made only of object types.
    fn object_like(&self, ty: TypeId) -> bool {
        if self.model.has_type_flags(ty, TypeFlags::OBJECT) {
            return true;
        }
        match self.model.intersection_constituents(ty) {
            Some(types) => types.iter().all(|&ty| self.object_like(ty)),
            None => false,
        }
    }

    /// Whether `construction_instance` recognizes every instantiation of a
    /// generic declaration's alternative.
    fn instance_provenance(
        &mut self,
        alternative: TypeId,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        Ok(!self
            .could_contain_type_variables_completion(alternative, consumer)?
            .admit()?
            || self
                .model
                .object_flags(alternative)?
                .intersects(ObjectFlags::REFERENCE)
            || self.model.metadata(alternative)?.alias.is_some()
            || self.anonymous_alternative(alternative)?)
    }

    fn anonymous_alternative(&self, ty: TypeId) -> Result<bool, DemandError> {
        Ok(self.model.has_type_flags(ty, TypeFlags::OBJECT)
            && self
                .model
                .object_flags(ty)?
                .intersects(ObjectFlags::ANONYMOUS))
    }

    /// Q5's per-form instance identity. A non-generic target's alternatives
    /// are interned identities. A generic declaration's alternative is
    /// recognized through the provenance TypeScript keeps on instantiations:
    /// identity when it contains no type variables, a reference's target, its
    /// own alias symbol, or an anonymous instantiation's target
    /// (`findMatchingTypeReferenceOrTypeAliasReference`'s two questions, plus
    /// identity and `instantiateAnonymousType`'s target).
    fn construction_instance(
        &mut self,
        source: TypeId,
        alternative: TypeId,
        generic: bool,
        consumer: &Location,
    ) -> Result<bool, DemandError> {
        if source == alternative {
            return Ok(true);
        }
        if !generic
            || !self
                .could_contain_type_variables_completion(alternative, consumer)?
                .admit()?
        {
            return Ok(false);
        }
        if self
            .model
            .object_flags(alternative)?
            .intersects(ObjectFlags::REFERENCE)
        {
            return Ok(self
                .model
                .object_flags(source)?
                .intersects(ObjectFlags::REFERENCE)
                && self.same_reference_target(source, alternative)?);
        }
        if self.model.metadata(alternative)?.alias.is_some() {
            return self.same_alias_symbol(source, alternative);
        }
        Ok(self.anonymous_alternative(alternative)?
            && self.anonymous_instantiation_target(source) == Some(alternative))
    }

    /// Whether a literal is in the default universe
    /// (`LiteralUniverse::TargetKeys`), from its members' keys
    /// (`object_member_key`) without typing any initializer: one that names a key some alternative
    /// declares, or names none (only spreads and late-bound names, or no
    /// member at all). A literal naming only other keys is outside it.
    pub(in crate::project::semantic) fn in_default_universe(
        &mut self,
        root: ExpressionRoot,
        target: &ConstructionTarget,
    ) -> Result<bool, DemandError> {
        let Some(keys) = &target.keys else {
            return Ok(true);
        };
        let consumer = super::super::location(self.inputs, root.file, root.site.span);
        let recipe = self.source.object_literal_recipe(root)?;
        let mut named = false;
        for member in recipe.members.iter().filter(|member| named_member(member)) {
            named = true;
            if keys.contains(&self.object_member_key(member, &consumer)?) {
                return Ok(true);
            }
        }
        Ok(!named)
    }

    /// The construction question for one examined literal: None when it is an
    /// assignment pattern, a spread contribution, has no context, or
    /// constructs no target alternative.
    pub(in crate::project::semantic) fn construction_at(
        &mut self,
        root: ExpressionRoot,
        target: &ConstructionTarget,
    ) -> Result<Option<ConstructionFacts>, DemandError> {
        let consumer = super::super::location(self.inputs, root.file, root.site.span);
        let recipe = self.source.object_literal_recipe(root)?;
        if recipe.destructuring || self.spread_contribution(root)? {
            return Ok(None);
        }
        let node = ContextNode::Captured(root);
        let Some(instantiated) = self
            .instantiated_contextual_type_completion(node, ContextFlags::NONE, &consumer)?
            .admit()?
        else {
            return Ok(None);
        };
        // Identity is decided on each constituent of the instantiated context:
        // its apparent form, which discrimination reads, can be a rebuilt
        // intersection or reference without the declared identity
        // (`getTypeWithThisArgument`). A constituent that is not an instance
        // is tried through its base constraint when it is instantiable (a
        // type parameter, `this`), and through its apparent forms otherwise.
        // Each match carries the apparent forms discrimination must keep; an
        // alternative reached twice is one construction.
        let mut matched: Vec<(TypeId, usize, Vec<TypeId>)> = Vec::new();
        for declared in self.constituents(instantiated) {
            let apparent = self
                .contextual_apparent_type_completion(declared, &consumer)?
                .admit()?;
            let apparent_forms = self.constituents(apparent);
            let candidates = if let Some(alternative) =
                self.matching_alternative(declared, target, &consumer)?
            {
                vec![(declared, alternative, apparent_forms)]
            } else if apparent == declared {
                Vec::new()
            } else {
                let identities = if self.model.has_type_flags(declared, TypeFlags::INSTANTIABLE) {
                    match self.base_constraint(declared, &consumer)? {
                        Some(constraint) => self.constituents(constraint),
                        None => Vec::new(),
                    }
                } else {
                    apparent_forms.clone()
                };
                let mut candidates = Vec::new();
                for form in identities {
                    if let Some(alternative) = self.matching_alternative(form, target, &consumer)? {
                        let forms = if apparent_forms.contains(&form) {
                            vec![form]
                        } else {
                            apparent_forms.clone()
                        };
                        candidates.push((form, alternative, forms));
                    }
                }
                candidates
            };
            for candidate in candidates {
                if !matched
                    .iter()
                    .any(|found| found.0 == candidate.0 && found.1 == candidate.1)
                {
                    matched.push(candidate);
                }
            }
        }
        // Discrimination keeps a subset of the constituents, so a context
        // with no target instance is decided before it is asked.
        if matched.is_empty() {
            return Ok(None);
        }
        let undiscriminated = self
            .map_type_no_reduction_completion(instantiated, &consumer, |checker, ty| {
                checker.contextual_apparent_type_completion(ty, &consumer)
            })?
            .admit()?;
        let discriminated = if self.model.has_type_flags(undiscriminated, TypeFlags::UNION) {
            self.discriminate_contextual_object_completion(root, undiscriminated, &consumer)?
                .admit()?
        } else {
            undiscriminated
        };
        let remaining = self.constituents(discriminated);
        let mut provided = None;
        let mut constructs = Vec::new();
        for (instance, alternative, forms) in matched {
            if !forms.iter().any(|form| remaining.contains(form)) {
                continue;
            }
            let provided = match &provided {
                Some(provided) => provided,
                None => provided.insert(self.provided_by_literal(root, &recipe, &consumer)?),
            };
            let lacks = self.construction_lacks(instance, provided, &consumer)?;
            constructs.push(ConstructedAlternative {
                alternative,
                instance,
                lacks,
            });
        }
        // The context as discrimination leaves it: the instantiated type, or
        // the discriminated apparent type when it removed a member.
        let contextual = if discriminated == undiscriminated {
            instantiated
        } else {
            discriminated
        };
        Ok((!constructs.is_empty()).then_some(ConstructionFacts {
            contextual,
            constructs,
        }))
    }

    /// The first target alternative `ty` is an instance of.
    fn matching_alternative(
        &mut self,
        ty: TypeId,
        target: &ConstructionTarget,
        consumer: &Location,
    ) -> Result<Option<usize>, DemandError> {
        for (index, &alternative) in target.alternatives.iter().enumerate() {
            if self.construction_instance(ty, alternative, target.generic, consumer)? {
                return Ok(Some(index));
            }
        }
        Ok(None)
    }

    fn constituents(&self, ty: TypeId) -> Vec<TypeId> {
        match self.model.union_constituents(ty) {
            Some(types) => types.to_vec(),
            None => vec![ty],
        }
    }

    /// The checked literal type's named keys, without typing initializers
    /// where that is exact (`written_keys`); otherwise the checked type,
    /// which a spread's private suppression and generic reduction need.
    fn provided_by_literal(
        &mut self,
        root: ExpressionRoot,
        recipe: &ObjectLiteralRecipe,
        consumer: &Location,
    ) -> Result<Provided, DemandError> {
        Ok(match self.written_keys(recipe, consumer)? {
            Some(keys) => Provided::Written(keys),
            None => Provided::Checked(self.type_of_node_completion(root)?.admit()?),
        })
    }

    /// The keys a literal writes when its members alone decide them: with no
    /// spread and no late-bound name, its checked type owns exactly its
    /// members' keys (`object_member_key`) as required named properties and
    /// no index signature (`checkObjectLiteral`); lookup resolves any other
    /// key through the global Object type's named members. None when a member
    /// may write another key.
    fn written_keys(
        &mut self,
        recipe: &ObjectLiteralRecipe,
        consumer: &Location,
    ) -> Result<Option<Vec<NativePropertyKey>>, DemandError> {
        if !recipe.members.iter().all(named_member) {
            return Ok(None);
        }
        let mut keys = Vec::new();
        for member in &recipe.members {
            keys.push(self.object_member_key(member, consumer)?);
        }
        Ok(Some(keys))
    }

    /// Each of the alternative's properties the literal does not certainly
    /// provide, in property order, with `getUnmatchedPropertiesWorker`'s
    /// target-property test.
    fn construction_lacks(
        &mut self,
        alternative: TypeId,
        provided: &Provided,
        consumer: &Location,
    ) -> Result<Vec<LackFact>, DemandError> {
        let mut lacks = Vec::new();
        for property in self
            .properties_of_type_completion(alternative, consumer)?
            .admit()?
        {
            let Some((symbol, optional)) = self.unmatched_target_property(&property, consumer)?
            else {
                continue;
            };
            let key = self.property_lookup_key(symbol, consumer)?;
            let presence = match provided {
                Provided::Written(keys) if keys.contains(&key) => None,
                // `getPropertyOfType` falls back to the global Object type's
                // own named members (augmentations included) for a key the
                // literal does not own: Object's signatures reach no Function
                // member, and its index signatures are not the literal's.
                Provided::Written(_) => {
                    let object = self.global_object_type("Object", consumer)?;
                    match self.property_of_object_type_by_key(object, &key, consumer)? {
                        None => Some(Presence::Absent),
                        Some(property) => {
                            let symbol = self.property_symbol(&property, consumer)?;
                            self.property_symbol_flags(symbol)?
                                .optional
                                .then_some(Presence::Unknown)
                        }
                    }
                }
                Provided::Checked(literal) => self.checked_presence(*literal, &key, consumer)?,
            };
            if let Some(presence) = presence {
                lacks.push(LackFact {
                    key: property.key_snapshot(),
                    optional,
                    presence,
                });
            }
        }
        Ok(lacks)
    }

    /// A key in the checked literal type (None when certainly present): `any`
    /// is unknown; otherwise each union constituent (or the type) has it as a
    /// required or optional named property, or not at all, and an absent key
    /// that an index signature applies to (`getApplicableIndexInfoForName`)
    /// may be present. Only agreement on required, or on absent, is certain.
    fn checked_presence(
        &mut self,
        literal: TypeId,
        key: &NativePropertyKey,
        consumer: &Location,
    ) -> Result<Option<Presence>, DemandError> {
        if self.model.has_type_flags(literal, TypeFlags::ANY) {
            return Ok(Some(Presence::Unknown));
        }
        let types = self.constituents(literal);
        let (mut present, mut absent) = (0, 0);
        for &ty in &types {
            match self
                .property_of_type_by_key_completion(ty, key, consumer)?
                .admit()?
            {
                None => {
                    if self
                        .applicable_index_info_for_key(ty, key, consumer)?
                        .is_none()
                    {
                        absent += 1;
                    }
                }
                Some(property) => {
                    let symbol = self.property_symbol(&property, consumer)?;
                    if !self.property_symbol_flags(symbol)?.optional {
                        present += 1;
                    }
                }
            }
        }
        Ok(if present == types.len() {
            None
        } else if absent == types.len() {
            Some(Presence::Absent)
        } else {
            Some(Presence::Unknown)
        })
    }
}

/// A member whose key the binder names: not a spread, and with no dynamic
/// name (a late-bound name's key is a type's).
fn named_member(member: &ObjectMember) -> bool {
    member.kind != K::SpreadAssignment && !member.dynamic_name
}
