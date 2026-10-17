//! Freeze collection into two lifetimes. Published projects share semantic
//! inputs; refreshable sessions also retain the recipes needed to bind again.
//! Neither product contains project-local identities or mutable resolution.
use super::*;
use std::sync::Arc;

/// Immutable collection used by a new binding pass. File-local indices remain
/// valid when a later program assigns the file a different FileId.
#[derive(Debug, Default)]
pub(in crate::project::binding) struct Facts {
    pub(in crate::project::binding) shared: Arc<SharedFacts>,
    pub(in crate::project::binding) names: Box<[Name]>,
    pub(in crate::project::binding) refs: Box<[Ref]>,
    pub(in crate::project::binding) binding_order: Box<[BindingEntry]>,
    pub(in crate::project::binding) anonymous_signatures: HashMap<DeclId, DeclId>,
    pub(in crate::project::binding) mixed_exports: Box<[NameId]>,
    pub(in crate::project::binding) misplaced_modules: Box<[(Span, MisplacedModule)]>,
}

/// Facts both binding and published native queries read. Share this group as a
/// unit; an ordinary Project can drop binding-only recipes after publication.
#[derive(Debug, Default)]
pub(in crate::project::binding) struct SharedFacts {
    pub(in crate::project::binding) module_declarations: Box<[ModuleDeclaration]>,
    pub(in crate::project::binding) lookup_scopes: super::super::site_map::SiteMap<ScopeId>,
    pub(in crate::project::binding) conditional_scopes: HashMap<Span, ScopeId>,
    pub(in crate::project::binding) flow: Option<super::super::flow::Graph>,
    pub(in crate::project::binding) scopes: Box<[Scope]>,
    pub(in crate::project::binding) decls: Box<[Decl]>,
    pub(in crate::project::binding) aliases: Box<[Alias]>,
    pub(in crate::project::binding) stars: Box<[Star]>,
    pub(in crate::project::binding) ambient_modules: Box<[AmbientModule]>,
    pub(in crate::project::binding) instantiated_augmentations: Box<[RequestId]>,
}

impl std::ops::Deref for Facts {
    type Target = SharedFacts;

    fn deref(&self) -> &Self::Target {
        &self.shared
    }
}

impl From<CollectedFacts> for Facts {
    fn from(facts: CollectedFacts) -> Self {
        let CollectedFacts {
            module_declarations,
            lookup_scopes,
            conditional_scopes,
            flow,
            names,
            scopes,
            decls,
            binding_order,
            anonymous_signatures,
            refs,
            aliases,
            stars,
            ambient_modules,
            mixed_exports,
            misplaced_modules,
            instantiated_augmentations,
        } = facts;
        Self {
            shared: Arc::new(SharedFacts {
                module_declarations: module_declarations.into_boxed_slice(),
                lookup_scopes,
                conditional_scopes,
                flow,
                scopes: scopes.into_boxed_slice(),
                decls: decls.into_boxed_slice(),
                aliases: aliases.into_boxed_slice(),
                stars: stars.into_boxed_slice(),
                ambient_modules: ambient_modules.into_boxed_slice(),
                instantiated_augmentations: instantiated_augmentations.into_boxed_slice(),
            }),
            names: names.into_boxed_slice(),
            refs: refs.into_boxed_slice(),
            binding_order: binding_order.into_boxed_slice(),
            anonymous_signatures,
            mixed_exports: mixed_exports.into_boxed_slice(),
            misplaced_modules: misplaced_modules.into_boxed_slice(),
        }
    }
}
