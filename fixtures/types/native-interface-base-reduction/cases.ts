export {};
interface Left { left: 301; }
interface Right { right: 302; }
type Both = Left & Right;
interface Valid extends Both {}
declare const valid: Valid;
const /*#valid*/validValue = valid.left;
type Alias = Both;
interface Aliased extends Alias {}
declare const aliased: Aliased;
const /*#alias*/aliasValue = aliased.right;
type Generic<T> = Left & { generic: T };
interface Instantiated extends Generic<303> {}
declare const instantiated: Instantiated;
const /*#generic*/genericValue = instantiated.generic;
type Nested = Both & { nested: 304 };
interface NestedBase extends Nested {}
declare const nested: NestedBase;
const /*#nested*/nestedValue = nested.nested;
type Empty = Left & {};
interface EmptyBase extends Empty {}
declare const empty: EmptyBase;
const /*#empty*/emptyValue = empty.left;
type Any = Left & any;
interface AnyBase extends Any { own: 305; }
declare const anyBase: AnyBase;
const /*#any*/anyValue = anyBase.own;
type NonPrimitive = object;
interface NonPrimitiveBase extends NonPrimitive { own: 306; }
declare const nonPrimitive: NonPrimitiveBase;
const /*#nonPrimitive*/nonPrimitiveValue = nonPrimitive.own;
type PrimitiveMix = Left & number;
interface PrimitiveBase extends PrimitiveMix { own: 307; }
declare const primitive: PrimitiveBase;
const /*#primitive*/primitiveValue = primitive.own;
type Union = Left | Right;
interface UnionBase extends Union { own: 308; }
declare const union: UnionBase;
const /*#union*/unionValue = union.own;
type Discriminants = { kind: 'left' } & { kind: 'right' };
interface DiscriminantBase extends Discriminants { own: 309; }
declare const discriminants: DiscriminantBase;
const /*#discriminants*/discriminantValue = discriminants.own;
type ConflictingProperty = { property: string } & { property: number };
interface PropertyBase extends ConflictingProperty { own: 310; }
declare const conflicting: PropertyBase;
const /*#propertyOwn*/propertyOwnValue = conflicting.own;
const /*#propertyNever*/propertyNeverValue = conflicting.property;
class PrivateLeft { private conflict!: string; }
class PrivateRight { private conflict!: number; }
type PrivateConflict = PrivateLeft & PrivateRight;
interface PrivateBase extends PrivateConflict { own: 311; }
declare const privateBase: PrivateBase;
const /*#private*/privateValue = privateBase.own;
type Mapping<T> = { [K in keyof T]: T[K] };
interface ConcreteMapped extends Mapping<Left> {}
declare const mapped: ConcreteMapped;
const /*#mapped*/mappedValue = mapped.left;
interface GenericMapped<T> extends Mapping<T> { own: 312; }
declare const genericMapped: GenericMapped<Left>;
const /*#genericMapped*/genericMappedValue = genericMapped.own;
interface Constrained<T extends Left> extends T { own: 313; }
declare const constrained: Constrained<Left>;
const /*#constrained*/constrainedValue = constrained.left;
interface Unconstrained<T> extends T { own: 314; }
declare const unconstrained: Unconstrained<Left>;
const /*#unconstrained*/unconstrainedValue = unconstrained.own;
interface ErrorThenValid extends MissingInterfaceBase, Left {}
declare const errorThenValid: ErrorThenValid;
const /*#errorThenValid*/errorThenValidValue = errorThenValid.left;
interface MergedRecovery extends MissingFirstBase { own: 315; }
interface MergedRecovery extends Left, MissingSecondBase {}
declare const mergedRecovery: MergedRecovery;
const /*#mergedRecovery*/mergedRecoveryValue = mergedRecovery.left;
interface SelfCycle extends SelfCycle { own: 316; }
declare const selfCycle: SelfCycle;
const /*#selfCycle*/selfCycleValue = selfCycle.own;
interface MutualLeft extends MutualRight { own: 317; }
interface MutualRight extends MutualLeft { other: 318; }
declare const mutual: MutualLeft;
const /*#mutualCycle*/mutualValue = mutual.own;
type IntersectionCycleBase = IntersectionCycle & Left;
interface IntersectionCycle extends IntersectionCycleBase { own: 319; }
declare const intersectionCycle: IntersectionCycle;
const /*#intersectionCycle*/intersectionCycleValue = intersectionCycle.own;
interface DefaultBase<T = DefaultDerived['prop']> { prop: T; }
interface DefaultDerived extends DefaultBase { own: 320; }
declare const defaultCycle: DefaultDerived;
const /*#defaultCycle*/defaultCycleValue = defaultCycle.own;
type ReducedUnion = Both | Discriminants;
interface ReducedUnionBase extends ReducedUnion {}
declare const reducedUnion: ReducedUnionBase;
const /*#reducedUnion*/reducedUnionValue = reducedUnion.right;
const Checked = class implements Valid { left!: 301; right!: 302; static value: 321 = 321; };
const /*#classDemand*/classDemandValue = Checked.value;
