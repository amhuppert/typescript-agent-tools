# Ordinary conditional diagnostic rendering

The selected missing-property access forces `reportNonexistentProperty` to
render its containing type. The inline, infer-context, and nondistributive rows
observe each missing true branch followed by its missing false branch before
the access returns native errorType. The named alias row observes neither
branch, because the same diagnostic can display `Named<T>` without expanding
its body. The healthy row returns 17.

All failing rows first observe another native errorType: `typeHasStaticProperty`
asks `getTypeOfSymbol` of the containing TypeLiteral symbol, whose non-value
flags reach the native fall-through. This precedes conditional rendering. A
public regression must preserve that first failure, both acquired branches,
and the access's own final failure; it must not assume the true branch is the
primary failure. The earlier unbound recovery becomes located at the consuming
access expression, while the access owner reports its own failure at `absent`.

The native check type remains a TypeParameter in the distributive rows, so these
controls reach the ordinary `conditionalTypeToTypeNode` arm, not its separate
GenerateNamesForShadowedTypeParams rewrite. The infer-context row also renders
the inferred U in the true tuple after restoring the surrounding infer-parameter
context. Captures run cold under strict, loose, and noCheck and check public API
parity. Hook event counts describe native acquisitions, not product cause totals.

These controls establish no evidence for the distinct synthetic reporting clone
in `reportErrorResults`; that owner remains separately planned.
