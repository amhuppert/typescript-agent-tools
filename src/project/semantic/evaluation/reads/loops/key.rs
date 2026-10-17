//! Flow cache identity follows the documented opaque-identity exception;
//! adjacent native name bytes still concatenate without path segmentation.

use crate::project::binding::{
    NativePropertyKey, NativeStringPart, NativeSymbolIdentity, flow::Site,
};
use crate::project::semantic::model::{Model, TypeId};
use crate::project::sources::FileId;

#[derive(Clone, PartialEq, Eq, Hash)]
enum Component {
    Symbol(NativeSymbolIdentity),
    Type(TypeId),
    Node(FileId, Site),
    Bytes(Vec<u8>),
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(in crate::project::semantic::evaluation::reads) struct FlowCacheKey(Vec<Component>);

/// `keyBuilder` writes actual identity components instead of numeric process
/// IDs. Bytes coalesce across write calls: dots and literal property text keep
/// native concatenation, while user text cannot become an identity component.
#[derive(Default)]
pub(super) struct FlowCacheKeyBuilder {
    components: Vec<Component>,
}

impl FlowCacheKeyBuilder {
    /// `writeFlowCacheKey` appends the exact accessed property name. Generated
    /// symbol names reuse the same opaque atoms as the property's native key.
    pub(super) fn write_name(&mut self, model: &Model, name: &NativePropertyKey) {
        for part in name.string_value().parts() {
            match part {
                NativeStringPart::Bytes(bytes) => self.write_bytes(&bytes),
                NativeStringPart::Identity(symbol) => self.write_symbol(model, symbol),
            }
        }
    }

    pub(super) fn write_symbol(&mut self, model: &Model, symbol: NativeSymbolIdentity) {
        model.observe_symbol_identity(symbol);
        self.components.push(Component::Symbol(symbol));
    }

    pub(super) fn write_type(&mut self, ty: TypeId) {
        self.components.push(Component::Type(ty));
    }

    pub(super) fn write_node(&mut self, file: FileId, site: Site) {
        self.components.push(Component::Node(file, site));
    }

    pub(super) fn write_bytes(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        match self.components.last_mut() {
            Some(Component::Bytes(previous)) => previous.extend_from_slice(bytes),
            _ => self.components.push(Component::Bytes(bytes.to_vec())),
        }
    }

    pub(super) fn finish(self) -> FlowCacheKey {
        FlowCacheKey(self.components)
    }
}
