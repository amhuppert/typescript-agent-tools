struct CapturedSyntax<'s> {
    ast: Ast<'s>,
    parents: Vec<Option<NodeId>>,
    attachments: HashMap<AttachmentKey, Attachment>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum AttachmentRole {
    Node,
    Name,
    FirstToken,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct AttachmentKey {
    role: AttachmentRole,
    kind: SyntaxKind,
    start: usize,
    end: usize,
}

enum Attachment {
    Unique(NodeId),
    Ambiguous,
}

impl<'s> CapturedSyntax<'s> {
    /// Capture the final reachable tree once. These are source attachments,
    /// never another token/binding inventory or semantic identities. Parents,
    /// exact node sites, named declarations and reserved-signature anchors
    /// all describe the same accepted final nodes.
    fn new(ast: Ast<'s>) -> Self {
        let mut parents = vec![None; ast.nodes().len()];
        let mut attachments = HashMap::new();
        let mut insert = |role, kind, span: Span, id| {
            let key = AttachmentKey { role, kind, start: span.start, end: span.end };
            attachments.entry(key).and_modify(|entry| {
                if !matches!(entry, Attachment::Unique(previous) if *previous == id) {
                    *entry = Attachment::Ambiguous;
                }
            }).or_insert(Attachment::Unique(id));
        };
        for id in ast.walk() {
            let node = ast.node(id);
            insert(AttachmentRole::Node, node.syntax_kind, ast.token_span(id), id);
            if let Some(name) = syntax::id(node, "name") {
                insert(AttachmentRole::Name, node.syntax_kind, ast.token_span(name), id);
            }
            // The retained binder's unnamed reserved-signature associations
            // use getFirstToken, a distinct anchor from a whole-node site.
            if matches!(node.syntax_kind, SyntaxKind::CallSignature
                | SyntaxKind::ConstructSignature | SyntaxKind::IndexSignature)
            {
                insert(AttachmentRole::FirstToken, node.syntax_kind, ast.first_token_span(id), id);
            }
            for child in ast.children(id) { parents[child.index()] = Some(id); }
        }
        CapturedSyntax { ast, parents, attachments }
    }

    fn attachment(&self, role: AttachmentRole, kind: SyntaxKind, span: Span)
        -> Result<Option<NodeId>, AnalysisError> {
        let key = AttachmentKey { role, kind, start: span.start, end: span.end };
        match self.attachments.get(&key) {
            None => Ok(None),
            Some(Attachment::Unique(id)) => Ok(Some(*id)),
            Some(Attachment::Ambiguous) => Err(super::internal(
                "retained site has several final syntax/role attachments")),
        }
    }

    /// Attach one retained final-tree site by exact token range and syntax
    /// role. Distinct nodes may share a range; ambiguous same-role attachment
    /// is a failure rather than an arbitrary parser arena choice.
    fn node_at(&self, site: crate::project::binding::flow::Site) -> Result<NodeId, AnalysisError> {
        self.attachment(AttachmentRole::Node, site.kind, site.span)?
            .ok_or_else(|| super::internal("retained site has no final syntax/role attachment"))
    }

    /// Attach the actual own declaration family selected by the binder's
    /// native type/value question. Related declaration kinds do not become
    /// interchangeable merely because their names share a span.
    fn declaration_at(&self, kinds: &[SyntaxKind], anchor: Span)
        -> Result<Option<NodeId>, AnalysisError> {
        let mut found = None;
        for &kind in kinds {
            if let Some(id) = self.attachment(AttachmentRole::Name, kind, anchor)? {
                if found.is_some_and(|previous| previous != id) {
                    return Err(super::internal("canonical declaration has several final syntax/role attachments"));
                }
                found = Some(id);
            }
        }
        Ok(found)
    }

    /// Reserved Call/Construct/Index associations retain native getFirstToken
    /// rather than a fabricated declaration name or a nearest-token search.
    fn first_token_at(&self, kind: SyntaxKind, anchor: Span)
        -> Result<Option<NodeId>, AnalysisError> {
        self.attachment(AttachmentRole::FirstToken, kind, anchor)
    }
}

