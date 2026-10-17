//! Native lazy assignment marking. Source supplies actual ordered candidates;
//! the completed binding owner supplies canonical identities. No type question
//! is needed to establish assignment position or definite-assignment facts.

use super::super::super::model::{DemandError, NativeCompletion, NativeResult, NativeSequence};
use super::super::super::source::{AssignmentSite, DeclarationIdentity, ExpressionRoot};
use super::super::{Evaluator, ReferenceDeclaration, ReferenceSymbol};
use super::{AssignmentKind, is_parameter_or_mutable_local_variable};
use crate::project::binding::flow::Site;
use crate::project::binding::{NativeLookupMeaning, NativeSymbolIdentity};
use crate::project::sources::FileId;
use crate::references::Location;
use crate::syntax_kind::SyntaxKind;
use std::collections::HashMap;

type Root = (FileId, SyntaxKind, usize, usize);

#[derive(Clone, Copy)]
enum LastAssignment {
    Position(usize),
    // Native math.MaxInt32: a nested assignment or local value export can
    // never establish a past-last capture in an admitted source file.
    Outer,
}

#[derive(Default)]
struct AssignmentFacts {
    owner: Option<Root>,
    last: Option<LastAssignment>,
    definite: bool,
}

#[derive(Clone)]
enum MarkedRoot {
    Marking,
    Inherited(Root),
    Ready(NativeCompletion<()>),
    Failed(DemandError),
}

#[derive(Default)]
pub(super) struct CaptureCache {
    roots: HashMap<Root, MarkedRoot>,
    symbols: HashMap<NativeSymbolIdentity, AssignmentFacts>,
}

fn key(file: FileId, site: Site) -> Root {
    (file, site.kind, site.span.start, site.span.end)
}

impl Evaluator<'_, '_> {
    /// `isPastLastAssignment`: no assignment, or the actual untrimmed reference
    /// Pos is strictly greater than the recorded extended statement End.
    pub(super) fn is_past_last_assignment_completion(
        &mut self,
        symbol: ReferenceSymbol,
        declaration: DeclarationIdentity,
        file: FileId,
        reference: Site,
        consumer: &Location,
    ) -> NativeResult<bool> {
        self.ensure_assignments_marked_completion(symbol, declaration, consumer)?
            .and_then_value(|()| {
                let identity = self.reference_symbol_identity(symbol, consumer)?;
                match self
                    .reads
                    .capture
                    .symbols
                    .get(&identity)
                    .and_then(|facts| facts.last)
                {
                    None => Ok(true),
                    Some(LastAssignment::Outer) => Ok(false),
                    Some(LastAssignment::Position(position)) => {
                        Ok(position < self.source.flow_reference_position(file, reference)?)
                    }
                }
            })
    }

    /// `isSymbolAssignedDefinitely` distinguishes a simple/logical/for target
    /// from arithmetic compound and increment targets, even after Outer.
    pub(super) fn is_symbol_assigned_definitely_completion(
        &mut self,
        symbol: ReferenceSymbol,
        declaration: DeclarationIdentity,
        consumer: &Location,
    ) -> NativeResult<bool> {
        self.ensure_assignments_marked_completion(symbol, declaration, consumer)?
            .and_then_value(|()| {
                let identity = self.reference_symbol_identity(symbol, consumer)?;
                Ok(self
                    .reads
                    .capture
                    .symbols
                    .get(&identity)
                    .is_some_and(|facts| facts.definite))
            })
    }

    /// `isSymbolAssigned` observes any native last-assignment mark, independently
    /// from definite assignment and source-position comparison.
    pub(in crate::project::semantic::evaluation) fn is_symbol_assigned_completion(
        &mut self,
        symbol: ReferenceSymbol,
        declaration: DeclarationIdentity,
        consumer: &Location,
    ) -> NativeResult<bool> {
        self.ensure_assignments_marked_completion(symbol, declaration, consumer)?
            .and_then_value(|()| {
                let identity = self.reference_symbol_identity(symbol, consumer)?;
                Ok(self
                    .reads
                    .capture
                    .symbols
                    .get(&identity)
                    .is_some_and(|facts| facts.last.is_some()))
            })
    }

    /// `ensureAssignmentsMarked` / `hasParentWithAssignmentsMarked`: install
    /// the native root flag before the ordered walk. A marked ancestor already
    /// covered nested roots. A failed walk remains a failure, including partial
    /// facts it produced before the failure; it cannot become a marked success.
    fn ensure_assignments_marked_completion(
        &mut self,
        symbol: ReferenceSymbol,
        declaration: DeclarationIdentity,
        consumer: &Location,
    ) -> NativeResult<()> {
        let identity = self.reference_symbol_identity(symbol, consumer)?;
        let Some(scope) = self.source.assignment_scope(declaration)? else {
            return Ok(NativeCompletion::clean(()));
        };
        let root = key(declaration.file, scope.root);
        // An interrupted walk is never admitted through partially installed
        // native marks. A completed walk replays its ordered acquisitions.
        for site in std::iter::once(scope.root).chain(scope.ancestors.iter().copied()) {
            if let Some(MarkedRoot::Failed(failure)) =
                self.reads.capture.roots.get(&key(declaration.file, site))
            {
                return Err(failure.clone());
            }
        }
        if let Some(owner) = self
            .reads
            .capture
            .symbols
            .get(&identity)
            .filter(|facts| facts.last.is_some())
            .and_then(|facts| facts.owner)
        {
            // Native returns before setting this declaration's root flag.
            // Its existing symbol fact still depends on the producing walk.
            return self.assignment_root_completion(owner);
        }
        if self.reads.capture.roots.contains_key(&root) {
            return self.assignment_root_completion(root);
        }
        // In-progress ancestor inheritance is a link, never a permanently
        // clean completion copied before that walk ends.
        let owner = scope
            .ancestors
            .iter()
            .map(|&site| key(declaration.file, site))
            .find(|ancestor| self.reads.capture.roots.contains_key(ancestor));
        if let Some(owner) = owner {
            self.reads
                .capture
                .roots
                .insert(root, MarkedRoot::Inherited(owner));
            return self.assignment_root_completion(owner);
        }
        self.reads.capture.roots.insert(root, MarkedRoot::Marking);
        let result = self.mark_node_assignments_completion(declaration.file, scope.root, consumer);
        self.reads.capture.roots.insert(
            root,
            match &result {
                Ok(value) => MarkedRoot::Ready(value.clone()),
                Err(failure) => MarkedRoot::Failed(failure.clone()),
            },
        );
        result
    }

    /// Observe the actual assignment walk behind a native marked flag. Links
    /// are installed only to an already present owner; no checker work occurs
    /// while following them, including native in-progress reentry.
    fn assignment_root_completion(&self, mut root: Root) -> NativeResult<()> {
        loop {
            match self.reads.capture.roots.get(&root) {
                Some(MarkedRoot::Inherited(owner)) => root = *owner,
                Some(MarkedRoot::Marking) => return Ok(NativeCompletion::clean(())),
                Some(MarkedRoot::Ready(value)) => return Ok(value.clone()),
                Some(MarkedRoot::Failed(failure)) => return Err(failure.clone()),
                None => {
                    return Err(super::super::super::internal(
                        "assignment mark lost its owning walk",
                    )
                    .into());
                }
            }
        }
    }

    /// `isParameterOrMutableLocalVariable` inspects the actual ValueDeclaration,
    /// independent of which retained declaration originally supplied its name.
    pub(in crate::project::semantic::evaluation::reads) fn mutable_reference_declaration(
        &mut self,
        symbol: ReferenceSymbol,
        consumer: &Location,
    ) -> Result<Option<DeclarationIdentity>, DemandError> {
        let Some(ReferenceDeclaration::Captured(declaration)) =
            self.reference_symbol_value_declaration(symbol, consumer)?
        else {
            return Ok(None);
        };
        if !matches!(
            self.source.declaration_kind(declaration)?,
            SyntaxKind::VariableDeclaration | SyntaxKind::Parameter | SyntaxKind::BindingElement
        ) {
            return Ok(None);
        }
        let setup = self.source.variable_declaration_read_facts(declaration)?;
        Ok(is_parameter_or_mutable_local_variable(&setup).then_some(declaration))
    }

    /// `markNodeAssignmentsWorker`: the single canonical-symbol operation for
    /// assignment and raw local-export candidates. Actual ValueDeclaration
    /// decides mutability even when the same header also carries Alias.
    fn mark_node_assignments_completion(
        &mut self,
        file: FileId,
        root: Site,
        consumer: &Location,
    ) -> NativeResult<()> {
        let mut sequence = NativeSequence::new();
        let result = (|| {
            for candidate in self.source.assignment_sites(file, root)? {
                let (symbol, reference, kind) = match candidate {
                    AssignmentSite::Target(reference) => {
                        let syntax = self.source.flow_syntax(file, reference)?;
                        let kind = self.assignment_kind(file, syntax.assignment_target)?;
                        let symbol =
                            sequence.read(self.reference_resolved_identifier_symbol_completion(
                                ExpressionRoot {
                                    file,
                                    site: reference,
                                },
                                consumer,
                            ))?;
                        (Some(symbol), Some(reference), kind)
                    }
                    AssignmentSite::Export(root) => {
                        let anchor = self.source.reference_entity_name_anchor(root)?;
                        let (symbol, _) = sequence.read(self.reference_entity_at_completion(
                            root.file,
                            anchor,
                            false,
                            NativeLookupMeaning::VALUE,
                            true,
                            consumer,
                        ))?;
                        (symbol, None, AssignmentKind::None)
                    }
                };
                let Some(symbol) = symbol else {
                    continue;
                };
                let Some(declaration) = self.mutable_reference_declaration(symbol, consumer)?
                else {
                    continue;
                };
                let identity = self.reference_symbol_identity(symbol, consumer)?;
                let previous = self
                    .reads
                    .capture
                    .symbols
                    .get(&identity)
                    .and_then(|facts| facts.last);
                let position =
                    if matches!(previous, Some(LastAssignment::Outer)) || reference.is_none() {
                        LastAssignment::Outer
                    } else if let Some(reference) = reference {
                        match self
                            .source
                            .assignment_position(file, reference, declaration)?
                        {
                            Some(position) => LastAssignment::Position(position),
                            None => LastAssignment::Outer,
                        }
                    } else {
                        unreachable!("export candidate handled before assignment position")
                    };
                let facts = self.reads.capture.symbols.entry(identity).or_default();
                facts.owner = Some(key(file, root));
                facts.last = Some(position);
                facts.definite |= kind == AssignmentKind::Definite;
            }
            Ok(())
        })();
        sequence.finish(result)
    }
}
