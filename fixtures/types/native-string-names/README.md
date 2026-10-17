# Exact cooked names

Native scanner token bytes are the identity for property names, export names,
and ambient-module lookup. Lone high/low escapes and five line-continuation
forms remain distinct from a paired scalar. Conditional property/export queries
expose that distinction; module queries resolve each exact ambient spelling.
Adjacent paired escapes and a written supplementary scalar are admitted controls.

The Rust identity contract permits scalar names. Module/export names report
non-scalar identity as Unsupported during Project::load; property names retain
their existing unsupported boundary when a query demands the name. Isolated
tests prove each variant at that actual boundary so an earlier failure cannot
hide acceptance of another name. The public AST's UTF-16 projection is not an
exact native name adapter.

The CR, CRLF, U+2028 and U+2029 fixture bytes are deliberate. Do not normalize
line endings in these source files.
