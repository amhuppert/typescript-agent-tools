//! Which type-argument attempts in expressions can close.
//!
//! At a `<` after an expression, TypeScript first tries to read type
//! arguments (`parseTypeArgumentsInExpression`). Every type-argument list
//! ends at its own `>`, so one forward scan of the tokens rules out attempts,
//! and nested lists inside them, that cannot close. Skipping those changes no
//! result; it keeps a long flat list such as `f(x < 0, x < 1, …)` from
//! reading each later item as a further nested type argument and charging
//! the depth budget per item.
//!
//! - **Strict:** a valid list pairs each `<` it contains with a `>`, and each
//!   bracket with its closer, so matching each `>` to the innermost open `<`
//!   decides whether a strict list can close.
//! - **Region:** no type parse, strict or recovering, passes an unbalanced
//!   `)`, `]`, or `}`, a `;` outside brackets, a token the lexer rejects, or
//!   the end of input. Recovery can skip `(` and `<` inside a parameter list
//!   and omit a type-parameter list's `>`, but each open type-argument list
//!   still consumes a distinct `>` before that stop.

use crate::lexer::{Kind, Lexer};
use crate::resources::{Control, Stop};
use std::collections::HashMap;

/// What the scan established about a `<`.
#[derive(Clone, Copy)]
pub(super) struct Angle {
    /// A strict type-argument list opened here may close.
    pub(super) strict: bool,
    /// The offset no type parse that starts here can pass.
    pub(super) stop: usize,
}

/// Facts about the source, filled lazily and kept across rollback.
#[derive(Default)]
pub(super) struct Angles {
    angles: HashMap<usize, Angle>,
    /// Offsets of every `>` the scans passed, one per character of a joined
    /// token, sorted.
    greater: Vec<usize>,
}

impl Angles {
    /// The facts for a `<` at `offset`, scanning from it when no earlier
    /// scan passed it.
    pub(super) fn at(
        &mut self,
        src: &str,
        offset: usize,
        control: Option<&Control>,
    ) -> Result<Angle, Stop> {
        if let Some(&angle) = self.angles.get(&offset) {
            return Ok(angle);
        }
        self.scan(src, offset, control)?;
        Ok(self.angles.get(&offset).copied().unwrap_or(Angle {
            strict: false,
            stop: offset,
        }))
    }

    /// The `>` characters after `from` and before `stop`: the most lists
    /// open at `from` that can still close.
    pub(super) fn closers(&self, from: usize, stop: usize) -> usize {
        let first = self.greater.partition_point(|&offset| offset <= from);
        let end = self.greater.partition_point(|&offset| offset < stop);
        end.saturating_sub(first)
    }

    fn scan(&mut self, src: &str, start: usize, control: Option<&Control>) -> Result<(), Stop> {
        #[derive(PartialEq)]
        enum Bracket {
            Group(u8),
            Template,
        }
        let mut lexer = Lexer::new(src, start);
        // Open `<` tokens not yet matched, with their bracket depth.
        let mut open: Vec<(usize, usize)> = Vec::new();
        // The `<` tokens opened at each bracket level, awaiting their stop.
        let mut levels: Vec<Vec<usize>> = Vec::new();
        levels.push(Vec::new());
        let mut brackets: Vec<Bracket> = Vec::new();
        let stop = loop {
            let token = lexer.next_controlled(control)?;
            if token.error.is_some() || token.kind == Kind::Eof {
                break token.start;
            }
            // Where a region of nested `<` tokens ends, and whether another
            // region opens at the same depth (the next template substitution).
            let mut close: Option<(usize, bool)> = None;
            match (token.kind, token.raw) {
                (Kind::TemplateHead, _) => {
                    brackets.push(Bracket::Template);
                    levels.push(Vec::new());
                }
                (Kind::Punct, "<" | "<<") => {
                    for offset in token.start..token.start + token.raw.len() {
                        open.push((offset, brackets.len()));
                        (levels.last_mut().unwrap()).push(offset);
                        self.angles.insert(
                            offset,
                            Angle {
                                strict: false,
                                stop: 0,
                            },
                        );
                    }
                }
                (Kind::Punct, "(") => {
                    brackets.push(Bracket::Group(b')'));
                    levels.push(Vec::new());
                }
                (Kind::Punct, "[") => {
                    brackets.push(Bracket::Group(b']'));
                    levels.push(Vec::new());
                }
                (Kind::Punct, "{") => {
                    brackets.push(Bracket::Group(b'}'));
                    levels.push(Vec::new());
                }
                (Kind::Punct, "}") if brackets.last() == Some(&Bracket::Template) => {
                    let continuation =
                        lexer.template_controlled(token.start, true, false, control)?;
                    if continuation.error.is_some() {
                        break token.start;
                    }
                    let tail = continuation.kind == Kind::TemplateTail;
                    if tail {
                        brackets.pop();
                    }
                    close = Some((token.start, !tail));
                }
                (Kind::Punct, ")" | "]" | "}") => {
                    if brackets.pop() != Some(Bracket::Group(token.raw.as_bytes()[0])) {
                        break token.start;
                    }
                    close = Some((token.start, false));
                }
                (Kind::Punct, ";") if brackets.is_empty() => break token.start,
                (Kind::Punct, raw) if raw.starts_with('>') => {
                    for (index, _) in raw.bytes().take_while(|&b| b == b'>').enumerate() {
                        self.add_greater(token.start + index)?;
                        if let Some((offset, _)) = open.pop() {
                            self.angles.get_mut(&offset).unwrap().strict = true;
                        }
                    }
                }
                _ => {}
            }
            if let Some((at, reopen)) = close {
                // A list opened inside the closed region cannot close outside
                // it.
                let depth = brackets.len() - usize::from(reopen);
                while open.last().is_some_and(|&(_, d)| d > depth) {
                    open.pop();
                }
                for offset in levels.pop().unwrap() {
                    self.angles.get_mut(&offset).unwrap().stop = at;
                }
                if reopen {
                    levels.push(Vec::new());
                }
            }
        };
        for level in levels {
            for offset in level {
                self.angles.get_mut(&offset).unwrap().stop = stop;
            }
        }
        Ok(())
    }

    fn add_greater(&mut self, offset: usize) -> Result<(), Stop> {
        match self.greater.last() {
            Some(&last) if last >= offset => {
                if let Err(index) = self.greater.binary_search(&offset) {
                    self.greater.insert(index, offset);
                }
            }
            _ => self.greater.push(offset),
        }
        Ok(())
    }
}
