//! Immutable flow attachment access. Binding lookup and source syntax
//! questions keep their distinct owners.

use super::TypeBindingFailure;
use super::flow::{Graph, Read, Site};
use super::inputs::BindingFiles;
use crate::project::sources::FileId;

impl<'a> BindingFiles<'a> {
    /// `getFlowNodeOfNode` observes the optional native field. Syntax callers
    /// already hold a captured node. Its explicit nil field differs from a
    /// site missing from the retained final tree.
    pub(in crate::project) fn flow_node_of_node(
        &self,
        file: FileId,
        site: Site,
    ) -> Result<Option<super::flow::FlowId>, TypeBindingFailure> {
        self.flow_attachment(file, site).map(|read| read.flow)
    }

    /// `getFlowNodeOfNode`: return the actual final-node binder attachment,
    /// including its explicit absent-flow state.
    pub(in crate::project) fn flow_attachment(
        &self,
        file: FileId,
        site: Site,
    ) -> Result<Read, TypeBindingFailure> {
        self.flow_graph(file)?
            .read(site)
            .ok_or(TypeBindingFailure::NotRetained)
    }

    pub(in crate::project) fn flow_graph(
        &self,
        file: FileId,
    ) -> Result<&'a Graph, TypeBindingFailure> {
        self.file(file)
            .flow()
            .ok_or(TypeBindingFailure::NotRetained)
    }

    pub(in crate::project) fn file_is_module(&self, file: FileId) -> bool {
        self.file(file).module()
    }
}
