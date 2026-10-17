# Checked construction and invocation dependencies

Operating envelope: native query checking over captured TypeScript source, in the
existing request-local worker. No execution or global project diagnostics.

Checked construction extends the existing native call owner. Source supplies actual constructor modifiers and class ancestry; evaluation acquires callee, signatures, accessibility, abstractness and overload choice in native order. Reusing the quick expression return would skip accessibility and argument dependencies; a separate overload engine would duplicate inference, arity and applicability. Keep resolveNewExpression as a complete dispatcher over the existing signature engine, with distinct resolveErrorCall child acquisition and checkCallExpression's call-signature-to-Any poststep.

Public/newable signatures, private/protected constructors within/nested/outside classes, inherited protected access (including intersection mixin filtering), abstract signatures/classes and union composites, genuine Any, call-only functions under noImplicitAny both ways, absent constructor signatures, wrong arity and consumed bad arguments/annotations all need pinned final-query evidence. TypeScript diagnostic TypeToString can consume dependencies before resolveErrorCall checks arguments; preserve those failures. No implicitAny diagnostic alone becomes compiler error-type unless native recovery actually supplies it.

The first implementation follows actual `resolveNewExpression`, `isConstructorAccessible`,
`typeHasProtectedAccessibleBase`, `someSignature`, `hasCorrectArity`, and
`invocationErrorDetails` owners. Missing argument-list presence is captured
separately from an empty list: the native missing-list branch returns directly
after its minimum-argument question, without asking rest or omitted types. Unknown signatures remain collected signatures until a
consumer demands their recovery return; a signature-only prelude is separate.

`getClassLikeDeclarationOfSymbol` uses the first actual class declaration, not
ValueDeclaration. Native diagnostic rendering uses the existing private byte
renderer, preserving its dependencies without publishing compiler prose.

The first native matrix shows why diagnostic-only and recovery paths must remain
separate. After a signature prelude reports inaccessible or abstract construction,
GetTypeAtLocation can still take the quick single-nongeneric constructor return.
The class type is then healthy; only a checked expression consumer demands the
unknown signature's error return. Call-only construction returns genuine Any;
noImplicitAny changes whether the selected call return is acquired first.

Optional calls use the same overload engine. `getOptionalCallSignature` owns
the cached signature clone and its inner/outer chain flags; the shared lazy
return operation applies the optional marker after target/composite/annotation
resolution and before popping the native return-resolution frame. Generic
instantiation therefore substitutes the target's transformed return rather
than independently interpreting call syntax. The access owner exposes the
whole `getOptionalExpressionType` and `propagateOptionalTypeMarker` operations
to access checking, call checking, effect lookup, and receiver checking.

The alternative of adding undefined at checkCallExpression's final return
would miss intermediate chain markers and signature consumers, and would
repeat the rule in quick queries. The quick path intentionally asks for a
single nongeneric signature on the original callee type, as the pinned native
compiler does; checked resolution first removes the optional marker. Tests
must cover both, including generic and overloaded calls, nested chains,
parenthesized calls, nullable return types, and explicit this parameters under
strict-null checking both ways.

Adversarial follow-up identified an additional existing checkCallExpression
obligation: `checkDeprecatedSignature` runs before new call-only handling and
the selected return. Its diagnostic `signatureToString` may acquire lazy
signature children. A diagnostic-only output is not evidence that its demands
can be skipped; the deprecated declaration and shared signature display owners
must be wired and tested. Import invocation recovery likewise must use the
shared `getSignaturesOfType` operation, including apparent/reduced acquisition,
rather than calling the narrower structured-signature table directly.
