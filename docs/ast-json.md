# JSON AST contract

The root is a `SourceFile` object. Each syntax node has:

| Field | Meaning |
| --- | --- |
| `kind` | Numeric TypeScript `SyntaxKind`, using the TypeScript 6.0.3 compiler API values. See [syntax-kinds.json](syntax-kinds.json). |
| `pos` | Inclusive **UTF-16** offset including preceding trivia, following compiler API conventions. |
| `end` | Exclusive UTF-16 offset. |
| `flags` | Compiler node flags for parsed contexts, declarations, optional chains, and related syntax. |
| Other fields | Compiler field names containing child nodes, arrays of nodes, booleans, numbers, or text. |

For example, a binary expression has `left`, `operatorToken`, and `right`; a variable declaration has `name`, optional `type`, and optional `initializer`. Identifier text is in `escapedText`, including TypeScript's escaping of names starting with two underscores. Numeric and string literal values are in `text`. Templates include cooked `text`, `rawText`, and `templateFlags`.

Strings and untagged templates reject malformed escapes. Tagged templates retain invalid escape text while cooking valid escapes before and after it, matching the compiler's syntax projection. Each segment accumulates its own escape flags; `hasExtendedUnicodeEscape` reflects valid extended Unicode escapes even when the segment also contains an invalid escape.

`SourceFile` includes `statements`, `endOfFileToken`, `text`, `fileName`, `languageVersion`, `languageVariant`, `scriptKind`, `isDeclarationFile`, and an empty `parseDiagnostics` array. Input errors produce a Rust `Diagnostic` or a nonzero CLI exit, not a partial `SourceFile`.

Ambiguous syntax can be reconsidered until an arrow, function type, or mapped type commits to its interpretation. Errors in the committed remainder, including nesting-budget errors, are returned directly rather than replaced by a fallback interpretation's diagnostic.

This is a serializable **syntax projection**, not a dump of an entire bound compiler object. It omits `parent` links, object IDs, transform/modifier caches, symbols, flow nodes, compiler maps, JSDoc trees, comment directives, and reference/pragma metadata. `HasAggregatedChildData` (2097152) is a lazy compiler cache flag and is excluded from differential comparisons. Arrays serialize as ordinary JSON arrays, as compiler `NodeArray` objects do through `JSON.stringify`. Import attributes use both `attributes` and the compiler's `assertClause` compatibility alias; both serialize the same child.

In Rust, `Node.kind` is the readable syntax name. `Node.span` and `Diagnostic.span` are **UTF-8 byte** ranges; use `Ast::utf16_offset` to convert a byte boundary within the original source to the JSON convention. Offsets outside the source or inside a UTF-8 character panic. `Ast::text` includes the node's preceding trivia. `Diagnostic::line_column` takes the source that produced the diagnostic and converts an error to one-based line and UTF-16 column, treating CRLF as one line break and recognizing CR, LF, U+2028, and U+2029.

`Ast` is immutable after parsing, preserving its child links, source spans, and cached position index. Read its source with `source()`, its allocated nodes with `nodes()`, and its root link with `root_id()`. These accessors replace the formerly mutable public `source`, `nodes`, and `root` fields.

`Node.fields` uses the public `Fields` alias, a small vector with one entry stored inline. Iterate over `(name, Value)` pairs or use `Node::field(name)` for lookup. Child links are arena `NodeId` values; `Ast::node(id)` returns the child and `Ast::view(id)` provides its nested serde view. An ID must come from the same arena; its numeric index alone does not establish ownership. Out-of-bounds lookups panic.

`Ast::write_json(writer, pretty)` is the recommended writer. It traverses the arena iteratively and preserves JavaScript strings containing unpaired UTF-16 surrogates as `\uXXXX` escapes. Those strings are `Value::Utf16` in Rust. `serde::Serialize` is also supported for normal-sized trees whose strings are valid Unicode; a serde serializer may recurse deeply or reject unpaired surrogates. The arena itself can be dropped without recursively traversing child nodes.

Arena nodes can include nodes made while disambiguating or reparsing syntax that are not reachable from the final root. Traverse from `Ast::root` using child fields when inspecting the parsed tree. JSON always traverses the final root.

## Compiler schema

Node construction uses a private typed `SyntaxKind`; serializers obtain its numeric value directly. Syntax kinds, node flags, scanner flags, and source-file metadata constants are generated together from the pinned TypeScript 6.0.3 API. The same generator writes [syntax-kinds.json](syntax-kinds.json), including compiler aliases. It refuses a different compiler version and supports a read-only drift check:

```sh
TYPE_SCRIPT_API=/path/to/typescript/lib/typescript.js \
  node tools/generate-compiler-schema.mjs
TYPE_SCRIPT_API=/path/to/typescript/lib/typescript.js \
  node tools/generate-compiler-schema.mjs --check
```
