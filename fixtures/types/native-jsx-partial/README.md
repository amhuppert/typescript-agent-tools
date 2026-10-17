# Native JSX partial-output control

TypeScript 7.0.2 gives the imported `Model` its declared type, the JSX value and
its property type `Element`, and the sibling property type `string`. The element
also exposes `tag: number`. JSX typing is explicitly deferred by this tool's
semantic profile, so this is a native-healthy control for partial expansion.
The final conditional type is an independent supported `number` control.

The configuration uses JSX preserve and noLib. Minimal ordinary globals and the
global JSX namespace make all native queries healthy without filtering missing
global diagnostics. Run the registered
`tools/type-native-jsx-partial-oracle.test.mjs` harness; it compares fresh native
checker results against the public semantic API and retains exact input hashes.
