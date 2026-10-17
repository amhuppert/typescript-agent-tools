# Native private property access

Pinned compiler: TypeScript 7.0.2, typescript-go commit
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b` (the harness also records the authoritative source commit).

This matrix selects actual property-access expressions, or an ordinary local
initializer for same-checker repeated helper requests. It compares each hooked
query with the ordinary public API before retaining its baseline. Separate query
requests do not establish a warm cache.

The native decisions are `checkPropertyAccessExpressionOrQualifiedName`,
`lookupSymbolForPrivateIdentifierDeclaration`,
`getPrivateIdentifierPropertyOfType`, `checkPrivateIdentifierPropertyAccess`,
`markLinkedReferences`, `isSelfTypeAccess`, and `checkExternalEmitHelpers`.

Core profiles distinguish default linked-reference marking from
`verbatimModuleSyntax`. They retain instance/static lexical tables, unrelated
textual keys, constructor readonly exemption, read/write accessors, methods,
generic and composite receiver types, and nested class/function scopes.
The diagnostic profile includes native error-type returns, the genuine Any
outside-class return, private shadowing versus outside-class diagnostics, and
lazy display dependencies. Diagnostic code arrays establish membership only;
ordered acquisition assertions must follow the native operation trace.

Helper profiles distinguish native target/define gates, disabled imports,
missing module/export, declaration signature versus variable call type,
Get/Set arity thresholds, aliased exports, and tuple-rest arity dependencies.
Compound access visits Set before Get. The repeat and write/read initializer
roots exercise the same evaluator's installed requested-helper bits.

Initial capture `vrun-3dd139e6-3a9c-433e-abd7-6b60d591e588` completed
all 116 public API/hook comparisons. Baselines are the measured capture files;
the initial groups failed only because those files did not yet exist. The
maintained comparison and final Project assertions are pending. No production
parity is claimed by a native baseline alone.

Zero-demand observations preserve the original short-circuit self-access
expression, Members-before-Exports lookup, and pre-existing alias-link
acquisition. They record raw fields and already acquired results only. The
trace proves that `Same.#token` marks the alias with default options and skips
that marking under verbatim; qualified `Exposed.Named` has a different self
comparison result. API selection can acquire one access more than once in the
same query, so trace multiplicity is retained rather than deduplicated.
