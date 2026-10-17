# Created property symbols

This extends the existing request-local property-symbol arena for native tuple
properties. The envelope remains one evaluator over an immutable Project. The
same property lookup, relation, flow, and presentation callers must consume the
actual symbol; a tuple-specific property descriptor would make each caller
reconstruct readonly, identity, instantiation, and raw/write semantics.

The current composite record conflates shared created-symbol metadata with
three value recipes. Adding tuple fields to that record would create invalid
combinations (a known tuple property with an invented containing union or clone
source). Instead, retain one `CreatedProperty` association containing actual
name/header, flags, check flags, declarations, value declaration, parent,
name type and mapper. Its value recipe distinguishes native deferred composite
values, `createSymbolWithType` clones, and factory-installed known values.
Own and instantiated associations stay separate. A created association carries
its metadata directly, removing the current parallel optional composite field.

`create_known_property(name, optional, readonly, type)` implements the symbol
creation sites in `createTupleTargetType`: a fresh transient Property symbol,
Optional as requested, Readonly check flag as requested, no parent/declarations,
and the supplied resolved raw type. It does not acquire SyntheticProperty flags.
Its numeric name and `length` use exact scalar text supplied by the tuple owner;
the property arena never derives identity from a displayed tuple label.

All created symbols use the existing read/write cache and property projection.
A Property write runs `getNonMissingTypeOfSymbol`, including exact optional
semantics; it does not add a tuple-specific write recipe. `instantiateSymbol`
observes the established resolved type for its identity-preserving fast path,
then creates a real fresh transient with target/mapper when required. Known
properties therefore participate in sorting, declarations, accessibility,
readonly display and relation comparison through the same operations as other
symbols. Composite constituents remain specific to the Composite recipe;
`forEachProperty` can only ask them after the Synthetic check-flag gate.

Validation reuses final composite/property flag/member serialization controls
for the representation migration, followed by native tuple final tests for
numeric/length reads and writes, readonly, optional Missing, actual transient
identity, and generic instantiation. These are behavior assertions, not tests
that freeze the internal record layout. No separate compiler identity or
presentation mapper is introduced.

## Unresolved cloned values

The public compiler control `type-native-property-clone-oracle.test.mjs` proves
that `this | Generic<T>` and `this | Interface<T>` can start with a canonical
(non-Transient) method. The clone then creates a fresh Anonymous whose actual
Type.symbol is the clone; the same-shaped instantiated constituents retain the
original anonymous type. Both plain and parenthesized annotations, strict and
loose null checks, are measured. The final consumer tests select property access
and qualified type-query names, with and without graph expansion. A registered
counterfactual will replace only the created Method factory call with its prior
explicit pending result, demonstrating that these final tests consume it.

A clone with an absent raw cache must execute getTypeOfSymbol for the clone.
Delegating Property or Accessor reads to its source is also incorrect: native
getTypeOfVariableOrParameterOrPropertyWorker and getTypeOfAccessors push a Type
resolution frame for the clone, and accessor completion preserves a type
installed during recursive work. Source write acquisition during clone creation
may warm the source cache without installing the clone's read cache.

Extend the existing workers with a small actual-value-symbol identity, Bound or
Created PropertySymbolId. It selects the existing cache and resolution target;
it does not select syntax or duplicate the read-family decision. The shared
NativePropertyFlags::value_kind owns Accessor before Variable|Property before
Method. An unresolved created read calls created_property_value(id, consumer),
which dispatches through that rule. Method uses the new anonymous factory;
Property and Accessor use the same workers as bound members with actual copied
ValueDeclaration and ordered declaration references. Unadmitted declaration
families keep their existing explicit dependency failures.

The arena needs complete TypeState observations and installations for created
read/write fields: Resolving and Failed are not native cache barriers;
Resolved and Recovered are. Resolution adds a PropertySymbol target carrying
Type or WriteType, and asks that installed-property observation before cycle
matching. No second result cache or guessed recovery bit is introduced. Ordinary
bound symbols keep their existing caches through the same value-symbol
operations. Rejected pushes add no frame; every accepted frame is popped even
when recipe acquisition fails. Accessor marked-pop recovery is installed before
returning its InvalidSemantic error; final cache completion preserves an actual
Resolved/Recovered installation, while a stopped native operation remains Failed.

Source shares the complete property-declaration recipe and accessor selection
owners over DeclarationReference (Bound or captured actual declaration). Getter,
setter, and auto-accessor annotation/body recipes remain distinct and lazy;
only the native selected dependency is evaluated. This preserves original
signature/declaration identities and error locations without treating copied
BoundSymbols as the clone's identity. Prototype dispatch uses the clone's actual
Parent before ordinary variable/property resolution, matching the native worker.
