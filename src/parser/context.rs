//! Grammar restrictions inherited by nested syntax. Compiler node flags are
//! derived here so grammar code does not need to encode their bit layout.

pub(super) use crate::compiler::node::{
    AMBIENT, AWAIT_CONTEXT as AWAIT, DECORATOR_CONTEXT as DECORATOR,
    DISALLOW_CONDITIONAL_TYPES_CONTEXT as DISALLOW_CONDITIONAL_TYPES,
    DISALLOW_IN_CONTEXT as DISALLOW_IN, IN_WITH_STATEMENT as IN_WITH, YIELD_CONTEXT as YIELD,
};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct GrammarContext {
    flags: u32,
    pub(super) allow_in: bool,
    // This restriction disambiguates an infer constraint before the enclosing
    // conditional type's '?'; it is distinct from the emitted context flag.
    pub(super) allow_conditional: bool,
}

impl GrammarContext {
    /// The context a file starts in: ambient in a declaration file, and
    /// `JavaScriptFile` in a JavaScript one, which every node then carries.
    pub(super) fn new(declaration_file: bool, javascript_file: bool) -> Self {
        let javascript = if javascript_file {
            crate::compiler::node::JAVA_SCRIPT_FILE
        } else {
            0
        };
        Self {
            flags: javascript | if declaration_file { AMBIENT } else { 0 },
            allow_in: true,
            allow_conditional: true,
        }
    }

    pub(super) fn contains(self, flags: u32) -> bool {
        self.flags & flags != 0
    }

    pub(super) fn with(mut self, flags: u32) -> Self {
        self.flags |= flags;
        self
    }

    pub(super) fn without(mut self, flags: u32) -> Self {
        self.flags &= !flags;
        self
    }

    pub(super) fn with_in(mut self, allowed: bool) -> Self {
        self.allow_in = allowed;
        self
    }

    pub(super) fn with_conditional(mut self, allowed: bool) -> Self {
        self.allow_conditional = allowed;
        self
    }

    /// Parameter decorators retain the enclosing decorator context and use the
    /// enclosing await context, while parameter syntax uses the function's.
    pub(super) fn parameters(self, async_function: bool, generator: bool) -> Self {
        self.without(AWAIT | YIELD)
            .with(if async_function { AWAIT } else { 0 })
            .with(if generator { YIELD } else { 0 })
    }

    pub(super) fn function(self, async_function: bool, generator: bool) -> Self {
        self.parameters(async_function, generator)
            .without(DECORATOR)
    }

    pub(super) fn node_flags(self) -> u32 {
        self.flags | if self.allow_in { 0 } else { DISALLOW_IN }
    }
}
