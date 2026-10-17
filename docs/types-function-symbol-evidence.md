# Function symbols and source-written names

This bounded Ousterhout checkpoint uses one immutable accepted Project and
request-local evaluator, with TypeScript 7.0.2 source at
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`. It closes two concrete display
defects; full callable, relation, context, order and resource obligations remain.

## Function serialization

Pinned `NodeBuilderImpl.shouldWriteTypeOfFunctionSymbol` at
`internal/checker/nodebuilderimpl.go:2733` distinguishes local functions from
nonlocal functions and static methods. Visited nonlocal functions can print
`typeof` their actual symbol. Local recursive functions print the finite native
`...` placeholder, distinct from the product's bounded-summary `…` elision.

`source/symbols.rs` retains Local or NonLocal with its source-written name for
the admitted canonical FunctionDeclaration-only family. Complete own
associations and actual final SourceFile/ModuleBlock parents establish this
family's nonlocal status. Function-expression/arrow variable rebinding,
static-method late-bindability and merged namespace structure remain required
in their own value constructors; none is admitted by this change.

`function_type` acquires structure and this fact together. Its evaluator
constructs one actual anonymous type in the canonical value cache and attaches
the fact to that identity. Model validates the attachment; presentation uses
it only at a visited identity. Alias and nominal metadata remain separate.
Renamed imports display the canonical function. Namespace members in the fixed
nil-enclosing context keep their native member spelling without fabricated
namespace qualification.

Native controls passed in `vrun-440f6a04-b69f-4b28-a377-5152fac60b4a`.
The public test's actual missing-summary RED was
`vrun-bceb5e88-2b2a-4517-b420-29ff3449de26`; its GREEN was
`vrun-77a96ddf-341a-4bcb-befb-c616d09a1b7b`, zero skips. Seven own/imported
names across strict/loose profiles and summary/expansion cover local,
top-level, private, namespace and escaped spellings. Recursive signature graph
edges reference the established root.

The final three-test call-cycle file passed with zero skips in
`vrun-65060c24-24f5-4b84-a73f-78438505bdca`, following original cycle RED
`vrun-b0643be8-86c4-4963-bb67-68e27829115d`. Intermediate
`vrun-267245da-48b7-4fec-95b2-4a1730761ccc` passed both actual SignatureId
mutual-return recovery/provenance tests but retained this display failure.
The final gate proves exact participant locations across option profiles and
summary/depth 0/3/32, healthy bare-self Never and established recursive
annotations with their native summary and graph self-edge.

## First own named association

Pinned `NodeBuilderImpl.getNameOfSymbolAsWritten` at line 967 reads the first
own declaration with a name, independently of the family supplying a type,
constraint or default. The binder already retains the canonical association
order. The shared source owner uses captured final declaration attachments and
delegates spelling to the existing `DeclarationNameToString` scanner owner.
Functions, declared targets and type parameters now share that decision.
Computed late/nameType and nameless synthetic branches require their own facts.

A preceding property and later defaulted parameter can share one native
Property|TypeParameter symbol:

```ts
interface Box { "T": number }
interface Box<T = unknown> {}
```

Its parameter summary is `"T"`; exact semantic name remains `T`. Previously
Source selected the display name after filtering to parameter recipes, printing
`T`. Display now consumes all own associations. Constraint/default/modifier
operations preserve their separately filtered native parameter lists. The
final graph keeps decoded name `T` and the later Unknown default.

An introduced required parameter hits the existing merged-parameter load rule;
initial run `vrun-707f267c-df87-4dee-ac18-79e3fb360aa4` was not a display RED.
After using the native allowed introduced default, compiler controls passed
in `vrun-3305e34c-787e-4c6e-b317-f2bf7dfc710d`. Actual public display RED
`vrun-334ad4a8-9446-4ded-a5a7-623f58df1a88` observed `T` versus `"T"`.
GREEN `vrun-ca2bc5d6-a7fb-469b-b2ed-ba9d8c55be96` has zero skips and
both profiles' bare/quoted/quoted-escaped/identifier-escaped first names,
decoded identity, independently retained default and final graph.

## Remaining limits

The shared source index records final node/name/first-token attachments once
per reprojected source; it is not a second binder or searched-token inventory.
Its mechanical migration and these gates do not establish memory/load cost or
close representation calibration. Remapped/default/nameless/computed symbol
rendering, expression/arrow/static-method eligibility and other visibility
contexts remain mandatory when those native families are admitted. No global
event or diagnostic alone invalidates an unrelated type.
