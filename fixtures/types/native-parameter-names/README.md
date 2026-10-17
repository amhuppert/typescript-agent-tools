The pinned native `symbolToParameterDeclaration` serializes the type before
`parameterToParameterDeclarationName`. The latter clones actual binding syntax;
the parameter symbol's internal name remains independent metadata.

These roots distinguish identifiers, decoded escapes, both quote kinds, numeric
literal text, nested/renamed/default/rest bindings, empty lists, omitted elements,
trailing commas, and literal/entity computed names. The removed initializer root
retains the native clone visitor's traversal before discarding the initializer.
Diagnostic-code arrays are sorted observations, not causal acquisition order.

The existing `native-this-expressions/pattern` root is the original final-query
regression (`({ value }?: ...)`, previously `(__0?: ...)`).
