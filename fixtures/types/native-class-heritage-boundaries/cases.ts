export {};
class Concrete { member: number = 1; static shared: string = ""; }
class OtherConcrete { member: number = 1; }
interface Contract { member: number; }
class Merged { member: number = 1; }
interface Merged { member: number; }
abstract class Abstract { abstract member: number; }
declare const WithInterface: new () => Concrete & Contract;
declare const WithConcrete: new () => Concrete & OtherConcrete;
declare const InterfaceOnly: new () => Contract;
declare const MappedOnly: new () => { [K in "member"]: number };
declare const StaticIntersection: (new () => Concrete) & { shared: string };
declare function makeBase(): typeof Concrete;
declare const holder: { Base: typeof Concrete };
type Primitive = string;
type Union = Contract | { other: number };
type Intersection = Contract & { other: number };
declare const BadReturn: new () => number;

const SyntheticInterface = class extends WithInterface { static value: 250 = 250; get member(): number { return 1; } };
const /*#syntheticInterface*/syntheticInterface = SyntheticInterface.value;
const SyntheticConcrete = class extends WithConcrete { static value: 251 = 251; get member(): number { return 1; } };
const /*#syntheticConcrete*/syntheticConcrete = SyntheticConcrete.value;
const OrdinaryMerged = class extends Merged { static value: 252 = 252; get member(): number { return 1; } };
const /*#ordinaryMerged*/ordinaryMerged = OrdinaryMerged.value;
const OrdinaryInterface = class extends InterfaceOnly { static value: 253 = 253; get member(): number { return 1; } };
const /*#ordinaryInterface*/ordinaryInterface = OrdinaryInterface.value;
const Mapped = class extends MappedOnly { static value: 254 = 254; get member(): number { return 1; } };
const /*#mapped*/mapped = Mapped.value;
const DeclaredOverride = class extends Concrete { static value: 255 = 255; declare member: number; constructor() { super(); this.member = 1; } };
const /*#declaredOverride*/declaredOverride = DeclaredOverride.value;
const AbstractOverride = class extends Concrete { static value: 256 = 256; abstract member: number; constructor() { super(); this.member = 1; } };
const /*#abstractOverride*/abstractOverride = AbstractOverride.value;
const AbstractImplementation = class extends Abstract { static value: 257 = 257; member: number = 1; };
const /*#abstractImplementation*/abstractImplementation = AbstractImplementation.value;
const AbstractRedeclaration = class extends Abstract { static value: 258 = 258; abstract member: number; };
const /*#abstractRedeclaration*/abstractRedeclaration = AbstractRedeclaration.value;
const StaticIntersectionOverride = class extends StaticIntersection { static value: 259 = 259; static shared: number = 1; };
const /*#staticIntersection*/staticIntersection = StaticIntersectionOverride.value;
const NullBase = class extends null { static value: 260 = 260; };
const /*#nullBase*/nullBase = NullBase.value;
const InvalidBase = class extends 42 { static value: 261 = 261; };
const /*#invalidBase*/invalidBase = InvalidBase.value;
const ArbitraryBase = class extends makeBase() { static value: 262 = 262; };
const /*#arbitraryBase*/arbitraryBase = ArbitraryBase.value;
const NonentityImplements = class implements makeBase() { static value: 263 = 263; };
const /*#nonentityImplements*/nonentityImplements = NonentityImplements.value;
const OptionalImplements = class implements holder?.Base { static value: 264 = 264; };
const /*#optionalImplements*/optionalImplements = OptionalImplements.value;
const ArityImplements = class implements Contract<number> { static value: 265 = 265; member: number = 1; };
const /*#arityImplements*/arityImplements = ArityImplements.value;
const PrimitiveImplements = class implements Primitive { static value: 266 = 266; };
const /*#primitiveImplements*/primitiveImplements = PrimitiveImplements.value;
const UnionImplements = class implements Union { static value: 267 = 267; member: number = 1; };
const /*#unionImplements*/unionImplements = UnionImplements.value;
const IntersectionImplements = class implements Intersection { static value: 268 = 268; member: number = 1; other: number = 1; };
const /*#intersectionImplements*/intersectionImplements = IntersectionImplements.value;
const InvalidReturn = class extends BadReturn { static value: 269 = 269; };
const /*#invalidReturn*/invalidReturn = InvalidReturn.value;
