These roots exercise `assignParameterType`, `assignBindingElementTypes`, and
`getTypeFromBindingPattern` through final type queries. Variance stays fixed
(`strictFunctionTypes: false`) while strict null checking and exact optional
properties vary independently.

The initial capture (vrun-490e3d0b-4ab3-4e99-b03f-cd081ad09075) showed that a
query on a callback call's result can use `quickTypeOfExpression` and skip
checking the callback. Five dependency rows therefore select actual bound
identifier reads inside their callbacks. `quickUnusedFields` retains the
clean call-result control. Expected dependency order follows the native
parameter assignment and binding-pattern traversal, not sorted diagnostic
arrays.

TypeScript 7.0.2 rejects `target: "es5"` with TS5108 before any semantic query.
The removed-target branch of the native implied-array operation is outside
this pinned compiler's accepted configuration envelope; this fixture does
not bypass the rejection. The accepted target controls use ESNext.
