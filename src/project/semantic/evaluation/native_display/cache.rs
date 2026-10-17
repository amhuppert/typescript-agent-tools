//! `visitAndTransformType` and its diagnostic `SymbolTrackerImpl` own one
//! request-local node-builder cache. Printer entries use InternalFlagsNone,
//! nil inner trackers and no hover expansion; Q4 budgets do not enter its key.
use super::super::symbol_accessibility::ScopeLocation;
use super::super::{ReferenceSymbol, SymbolMeaning};
use super::*;
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct CacheKey {
    ty: TypeId,
    flags: DiagnosticTypeFormat,
    // All current native printer entries pass InternalFlagsNone. This remains
    // distinct from FlagsWriteTypeParametersInQualifiedName (ordinary bit 9).
    internal_flags: u32,
}

pub(super) type SerializedTypes =
    HashMap<Option<ScopeLocation>, HashMap<CacheKey, Arc<SerializedType>>>;

pub(super) struct SerializedType {
    result: NativeCompletion<Node>,
    added_length: usize,
    truncating: bool,
    tracked_symbols: Vec<TrackedSymbol>,
}

#[derive(Clone, Copy)]
pub(super) struct TrackedSymbol {
    pub(super) symbol: ReferenceSymbol,
    pub(super) enclosing: Option<ScopeLocation>,
    pub(super) meaning: SymbolMeaning,
}

impl Evaluator<'_, '_> {
    /// `SymbolTrackerImpl.TrackSymbol` with the actual nil inner tracker used
    /// by the checker printers. It observes raw flags, never alias eligibility
    /// or symbol types. Cache replay repeats this recorder, not lookupSymbolChain
    /// or trackComputedName's earlier resolution.
    pub(super) fn diagnostic_track_symbol(
        &mut self,
        symbol: ReferenceSymbol,
        enclosing: Option<ScopeLocation>,
        meaning: SymbolMeaning,
        context: &mut Context,
        consumer: &Location,
    ) -> Result<(), DemandError> {
        let tracked = TrackedSymbol {
            symbol,
            enclosing,
            meaning,
        };
        if let Some(boundary) = context.recovery_boundaries.last_mut() {
            boundary.tracks.push(tracked);
        }
        if !self
            .reference_symbol_flags(symbol, consumer)?
            .type_parameter
        {
            context.tracked_symbols.push(TrackedSymbol {
                symbol,
                enclosing,
                meaning,
            });
        }
        Ok(())
    }

    /// `visitAndTransformType`: flags and actual enclosing identity partition
    /// the cache, whereas mapper/depth/current length deliberately do not.
    /// Native nil-enclosing transforms install entries but cannot read them.
    /// A completed recovery is cacheable; a builder diagnostic is a separate
    /// native veto. Interrupted work never installs a node or elision.
    pub(super) fn diagnostic_visit(
        &mut self,
        ty: TypeId,
        context: &mut Context,
        consumer: &Location,
        worker: fn(&mut Self, TypeId, &mut Context, &Location) -> NativeResult<Node>,
    ) -> NativeResult<Node> {
        let identity = if let Some(reference) = self.model.deferred_reference(ty) {
            Some(DepthIdentity::Node(reference.declaration))
        } else if let TypeView::Conditional(view) = self.model.get(ty)? {
            Some(DepthIdentity::Node(self.conditional_root_node(view.root)))
        } else {
            let constructor = self
                .model
                .object_flags(ty)?
                .intersects(ObjectFlags::ANONYMOUS)
                && self.type_symbol_flags(ty)?.is_some_and(|flags| flags.class);
            self.type_symbol_identity(ty)?
                .map(|symbol| DepthIdentity::Symbol(symbol, constructor))
        };
        let key = CacheKey {
            ty,
            flags: context.flags,
            internal_flags: 0,
        };
        // All diagnostic entries have native maxExpansionDepth == -1. Product
        // graph expansion is a different operation and never disables this cache.
        let cached = context.enclosing.and_then(|enclosing| {
            self.native_display
                .serialized
                .get(&Some(enclosing))
                .and_then(|entries| entries.get(&key))
                .cloned()
        });
        if let Some(cached) = cached {
            let replay = (|| {
                for tracked in &cached.tracked_symbols {
                    self.diagnostic_track_symbol(
                        tracked.symbol,
                        tracked.enclosing,
                        tracked.meaning,
                        context,
                        consumer,
                    )?;
                }
                context.truncating |= cached.truncating;
                context.length += cached.added_length;
                Ok(())
            })();
            // Node is owned, immutable output syntax. Clone is the native deep
            // clone, with no checker reads; raw source-free evidence is copied
            // and remains unobserved until the outer diagnostic entry returns.
            return cached
                .result
                .clone()
                .and_then_value(|node| replay.map(|()| node));
        }

        let depth = identity
            .and_then(|identity| context.depths.get(&identity).copied())
            .unwrap_or(0);
        if identity.is_some() && depth > 10 {
            context.elide();
            return Ok(NativeCompletion::clean(context.elision(None)));
        }
        if let Some(identity) = identity {
            context.depths.insert(identity, depth + 1);
        }
        context.visited.insert(ty);
        let previous_tracks = std::mem::take(&mut context.tracked_symbols);
        let start_length = context.length;
        let result = worker(self, ty, context, consumer);
        if let Ok(completed) = &result
            && !context.reported_diagnostic
            && !context.encountered_error
        {
            self.native_display
                .serialized
                .entry(context.enclosing)
                .or_default()
                .insert(
                    key,
                    Arc::new(SerializedType {
                        result: completed.clone(),
                        added_length: context.length - start_length,
                        truncating: context.truncating,
                        tracked_symbols: context.tracked_symbols.clone(),
                    }),
                );
        }
        context.visited.remove(&ty);
        if let Some(identity) = identity {
            if depth == 0 {
                context.depths.remove(&identity);
            } else {
                context.depths.insert(identity, depth);
            }
        }
        // Misses isolate child tracked lists; they do not append them to the
        // parent. Hits replay into the current list. Native distinguishes these.
        context.tracked_symbols = previous_tracks;
        result
    }
}
