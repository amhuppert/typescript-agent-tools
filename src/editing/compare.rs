//! Shared syntax correspondence: whether the syntax an edit preserves is
//! the same in the composed text's final tree, through the composition's
//! change map. Operation policies decide which slots an edit opens; this
//! module decides how everything around them corresponds, and returns what
//! fills each slot for the policy to judge.
//!
//! Nodes correspond by kind, logical fields in order, exact scalar values,
//! and flags; token positions correspond through the change map. The named
//! treatments of the design are the only exceptions:
//!
//! - token positions relate through `Composition::map`, and an enclosing
//!   span may end where a filled slot or an explicit seam edit ends;
//! - `SourceFile.text` is the composed text itself;
//! - the source file's `POSSIBLY_CONTAINS_DYNAMIC_IMPORT` and
//!   `POSSIBLY_CONTAINS_IMPORT_META` summaries describe the new file, since an
//!   opened slot may add or remove those forms; no other flag is excused;
//! - `assertClause` repeats `attributes`, so that child is compared once, as
//!   `Ast::children` traverses it;
//! - parser bookkeeping outside the syntax fields, such as
//!   `possible_top_level_await`, is not syntax and is not compared.

use super::compose::{Composition, Side};
use crate::compiler::node::{
    DEPRECATED, POSSIBLY_CONTAINS_DYNAMIC_IMPORT, POSSIBLY_CONTAINS_IMPORT_META,
};
use crate::syntax_kind::SyntaxKind;
use crate::{Ast, NodeId, Span, Value};

/// A place in the original tree an edit may fill with new syntax, which
/// must lie within `region` of the composed text.
#[derive(Debug, Clone)]
pub(super) enum Slot {
    /// A node's single-node field, replaced whole.
    Field {
        node: NodeId,
        field: &'static str,
        region: Span,
    },
    /// New items in a node's list field, before its item at `index` (its
    /// length for the end).
    Gap {
        node: NodeId,
        field: &'static str,
        index: usize,
        region: Span,
    },
}

/// A protected node whose end an explicit seam edit moves, such as a
/// statement that absorbs an inserted terminator, and where it ends in the
/// composed text.
#[derive(Debug, Clone, Copy)]
pub(super) struct Extension {
    pub(super) node: NodeId,
    pub(super) end: usize,
}

/// Preserved syntax that does not correspond: where it is in the original
/// text and in the composed text, and what differs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Mismatch {
    pub(super) original: usize,
    pub(super) composed: usize,
    pub(super) message: String,
}

/// Compare `original` with `composed`, the final tree of
/// `composition.text()`, outside `slots`. On success, the composed nodes
/// that fill each slot, in slot order.
pub(super) fn compare(
    original: &Ast<'_>,
    composed: &Ast<'_>,
    composition: &Composition,
    slots: &[Slot],
    extensions: &[Extension],
) -> Result<Vec<Vec<NodeId>>, Mismatch> {
    let mut comparison = Comparison {
        original,
        composed,
        composition,
        slots,
        extensions,
        fills: vec![Vec::new(); slots.len()],
        ends: Vec::new(),
        extended: Vec::new(),
    };
    let mut work = vec![(original.root_id(), composed.root_id())];
    while let Some((o, c)) = work.pop() {
        comparison.pair(o, c, &mut work)?;
    }
    comparison.check_ends()?;
    Ok(comparison.fills)
}

struct Comparison<'a, 'o, 'c> {
    original: &'a Ast<'o>,
    composed: &'a Ast<'c>,
    composition: &'a Composition,
    slots: &'a [Slot],
    extensions: &'a [Extension],
    fills: Vec<Vec<NodeId>>,
    /// Ends to check once every slot's fill is known: the original node,
    /// its expected composed end, and its actual one.
    ends: Vec<(NodeId, usize, usize)>,
    /// Original nodes that end with a list a gap extends at its end, such
    /// as a case clause after its last statement, and where they now end:
    /// with the gap's last fill.
    extended: Vec<(NodeId, usize)>,
}

impl Comparison<'_, '_, '_> {
    fn mismatch(&self, o: NodeId, c: NodeId, message: String) -> Mismatch {
        Mismatch {
            original: self.original.token_span(o).start,
            composed: self.composed.token_span(c).start,
            message,
        }
    }

    fn pair(
        &mut self,
        o: NodeId,
        c: NodeId,
        work: &mut Vec<(NodeId, NodeId)>,
    ) -> Result<(), Mismatch> {
        let (on, cn) = (self.original.node(o), self.composed.node(c));
        if on.syntax_kind != cn.syntax_kind {
            return Err(self.mismatch(o, c, format!("{} would become {}", on.kind, cn.kind)));
        }
        let excused = if on.syntax_kind == SyntaxKind::SourceFile {
            POSSIBLY_CONTAINS_DYNAMIC_IMPORT | POSSIBLY_CONTAINS_IMPORT_META
        } else {
            0
        };
        let changed = (on.flags ^ cn.flags) & !excused;
        if changed != 0 {
            let message = if changed == DEPRECATED {
                format!(
                    "{} would {} deprecated by a JSDoc @deprecated tag",
                    on.kind,
                    if cn.flags & DEPRECATED != 0 {
                        "become"
                    } else {
                        "no longer be"
                    }
                )
            } else {
                format!(
                    "{}'s flags would change from {} to {}",
                    on.kind, on.flags, cn.flags
                )
            };
            return Err(self.mismatch(o, c, message));
        }
        let start = self.original.token_span(o).start;
        let expected = self.composition.map(start, Side::After);
        if expected != Some(self.composed.token_span(c).start) {
            return Err(self.mismatch(o, c, format!("{} would start elsewhere", on.kind)));
        }
        // A list a gap opens contains what is inserted at its end, and a
        // zero-width node, such as the end-of-file token, lies wholly after
        // an insertion at its position, as its start does.
        let owns_gap = self
            .slots
            .iter()
            .any(|slot| matches!(slot, Slot::Gap { node, .. } if *node == o));
        let side = if owns_gap || start == on.span.end {
            Side::After
        } else {
            Side::Before
        };
        let end = match self.extensions.iter().find(|extension| extension.node == o) {
            Some(extension) => Some(extension.end),
            None => self.composition.map(on.span.end, side),
        };
        let Some(end) = end else {
            return Err(self.mismatch(o, c, format!("{} would end elsewhere", on.kind)));
        };
        self.ends.push((o, end, cn.span.end));
        let (ofields, cfields): (Vec<_>, Vec<_>) = (
            self.original.logical_fields(o).collect(),
            self.composed.logical_fields(c).collect(),
        );
        let names = |fields: &[(&'static str, &Value<'_>)]| -> Vec<&'static str> {
            fields.iter().map(|(name, _)| *name).collect()
        };
        if names(&ofields) != names(&cfields) {
            return Err(self.mismatch(o, c, format!("{}'s parts would change", on.kind)));
        }
        let fields = ofields.into_iter().zip(cfields);
        let mut children = Vec::new();
        for ((name, ov), (_, cv)) in fields {
            let name: &'static str = name;
            match (ov, cv) {
                (Value::Node(oc), Value::Node(cc)) => match self.slot(o, name) {
                    Some(slot) => self.fill_field(slot, o, *cc)?,
                    None => children.push((*oc, *cc)),
                },
                (Value::Nodes(os), Value::Nodes(cs)) => {
                    self.list(o, c, name, os, cs, &mut children)?;
                }
                (Value::Text(a), Value::Text(b)) => {
                    if on.syntax_kind == SyntaxKind::SourceFile && name == "text" {
                        if b.as_ref() != self.composition.text() {
                            return Err(self.mismatch(o, c, "the composed text differs".into()));
                        }
                    } else if a != b {
                        return Err(self.mismatch(
                            o,
                            c,
                            format!("{}'s {name} would change", on.kind),
                        ));
                    }
                }
                (Value::Utf16(a), Value::Utf16(b)) if a == b => {}
                (Value::Bool(a), Value::Bool(b)) if a == b => {}
                (Value::Number(a), Value::Number(b)) if a == b => {}
                _ => {
                    return Err(self.mismatch(o, c, format!("{}'s {name} would change", on.kind)));
                }
            }
        }
        // Visit children in source order.
        work.extend(children.into_iter().rev());
        Ok(())
    }

    /// The slot that opens `field` of the original `node`, if any.
    fn slot(&self, node: NodeId, field: &str) -> Option<usize> {
        self.slots.iter().position(|slot| match slot {
            Slot::Field {
                node: n, field: f, ..
            } => *n == node && *f == field,
            Slot::Gap { .. } => false,
        })
    }

    /// Whether composed syntax lies within `region`, outside which it may
    /// have only trivia.
    fn within(&self, c: NodeId, region: Span) -> bool {
        let span = self.composed.token_span(c);
        let text = self.composition.text();
        span.start >= region.start
            && span.end <= region.end
            && trivia_only(text, region.start, span.start)
            && trivia_only(text, span.end, region.end)
    }

    fn fill_field(&mut self, slot: usize, o: NodeId, c: NodeId) -> Result<(), Mismatch> {
        let Slot::Field { region, .. } = self.slots[slot] else {
            unreachable!("a field slot");
        };
        if !self.within(c, region) {
            return Err(self.mismatch(
                o,
                c,
                "the new syntax would not fill exactly the replaced slot".into(),
            ));
        }
        self.fills[slot].push(c);
        Ok(())
    }

    /// Compare a list field: item by item, or, where gaps open it, with the
    /// composed items inside a gap's region as that gap's fill.
    fn list(
        &mut self,
        o: NodeId,
        c: NodeId,
        name: &str,
        os: &[NodeId],
        cs: &[NodeId],
        children: &mut Vec<(NodeId, NodeId)>,
    ) -> Result<(), Mismatch> {
        let gaps: Vec<usize> = (0..self.slots.len())
            .filter(|&slot| {
                matches!(&self.slots[slot], Slot::Gap { node, field, .. } if *node == o && *field == name)
            })
            .collect();
        let kind = self.original.node(o).kind;
        let mut next = 0;
        for &item in cs {
            let span = self.composed.token_span(item);
            let gap = gaps.iter().copied().find(|&slot| {
                let Slot::Gap { region, .. } = self.slots[slot] else {
                    unreachable!("a gap slot");
                };
                span.start >= region.start && span.end <= region.end
            });
            match gap {
                Some(slot) => {
                    let Slot::Gap { index, .. } = self.slots[slot] else {
                        unreachable!("a gap slot");
                    };
                    if index != next {
                        return Err(self.mismatch(
                            o,
                            item,
                            format!("new syntax would land elsewhere in {kind}'s {name}"),
                        ));
                    }
                    self.fills[slot].push(item);
                }
                None => {
                    let Some(&original) = os.get(next) else {
                        return Err(self.mismatch(
                            o,
                            item,
                            format!("{kind}'s {name} would gain an item outside the edit"),
                        ));
                    };
                    children.push((original, item));
                    next += 1;
                }
            }
        }
        if next != os.len() {
            return Err(self.mismatch(os[next], c, format!("{kind}'s {name} would lose an item")));
        }
        // A node that ends with this list's last item, such as a case
        // clause, ends with what a gap at the list's end adds.
        let owner = self.original.node(o);
        let ends_with_list = os
            .last()
            .is_some_and(|&last| self.original.node(last).span.end == owner.span.end)
            && self
                .original
                .logical_fields(o)
                .filter(|(_, value)| matches!(value, Value::Node(_) | Value::Nodes(_)))
                .last()
                .is_some_and(|(field, _)| field == name);
        if ends_with_list {
            let end_fill = gaps.iter().find_map(|&slot| match &self.slots[slot] {
                Slot::Gap { index, .. } if *index == os.len() => self.fills[slot].last(),
                _ => None,
            });
            if let Some(&fill) = end_fill {
                self.extended.push((o, self.composed.node(fill).span.end));
            }
        }
        Ok(())
    }

    /// Check each protected node's end, now that every slot's fill is
    /// known: a node that ended with a replaced slot ends with its fill.
    fn check_ends(&self) -> Result<(), Mismatch> {
        for &(o, expected, actual) in &self.ends {
            if let Some(&(_, end)) = self.extended.iter().find(|(node, _)| *node == o) {
                if end != actual {
                    return Err(Mismatch {
                        original: self.original.token_span(o).start,
                        composed: actual,
                        message: format!("{} would end elsewhere", self.original.node(o).kind),
                    });
                }
                continue;
            }
            let expected = self
                .slots
                .iter()
                .zip(&self.fills)
                .find_map(|(slot, fill)| match (slot, fill.as_slice()) {
                    (Slot::Field { region, .. }, [node]) if region.end == expected => {
                        Some(self.composed.node(*node).span.end)
                    }
                    _ => None,
                })
                .unwrap_or(expected);
            if expected != actual {
                let node = self.original.node(o);
                return Err(Mismatch {
                    original: self.original.token_span(o).start,
                    composed: actual,
                    message: format!("{} would end elsewhere", node.kind),
                });
            }
        }
        Ok(())
    }
}

/// Whether `text[start..end]` holds only whitespace and comments.
fn trivia_only(text: &str, start: usize, end: usize) -> bool {
    if start >= end {
        return start == end;
    }
    let mut at = start;
    loop {
        let rest = &text[at..end];
        at = end - rest.trim_start_matches(crate::lexer::whitespace).len();
        if at == end {
            return true;
        }
        match crate::lexer::comment_end(text, at) {
            Some(comment) if comment <= end => at = comment,
            _ => return false,
        }
    }
}
