export {};
class Plain { member: number = 1; static shared: string = ""; }
class PrivateBase { private constructor() {} member: number = 1;
    static make() { const C = class extends PrivateBase { static value: 201 = 201; }; const /*#privateWithin*/privateWithin = C.value; }
}
class GenericBase<T extends string> { item!: T; }
abstract class AbstractBase { abstract member: number; }
class PropertyBase { member: number = 1; }
class AccessorBase { get member(): number { return 1; } }
class MethodBase { member(): number { return 1; } }
class PrivateMemberBase { private member: number = 1; }
interface Contract { member: number; }
interface GenericContract<T extends string> { member: T; }
interface BrokenContract { member: MissingContractType; }
declare const ConstructLike: { new(value: number): { member: number }; };
declare const UnequalConstructLike: { new(value: number): { first: number }; new(value: string): { second: string }; };
const DerivedPlain = class extends Plain { static value: 200 = 200; own: number = 1; };
const /*#plain*/plain = DerivedPlain.value;
const PrivateOutside = class extends PrivateBase { static value: 202 = 202; };
const /*#privateOutside*/privateOutside = PrivateOutside.value;
const GenericGood = class extends GenericBase<"ok"> { static value: 203 = 203; };
const /*#genericGood*/genericGood = GenericGood.value;
const GenericBad = class extends GenericBase<number> { static value: 204 = 204; };
const /*#genericBad*/genericBad = GenericBad.value;
const GenericMissing = class extends GenericBase<MissingBaseArgument> { static value: 205 = 205; };
const /*#genericMissing*/genericMissing = GenericMissing.value;
const InstanceMismatch = class extends Plain { static value: 206 = 206; member: string = ""; };
const /*#instanceMismatch*/instanceMismatch = InstanceMismatch.value;
const StaticMismatch = class extends Plain { static value: 207 = 207; static shared: number = 1; };
const /*#staticMismatch*/staticMismatch = StaticMismatch.value;
const ClassLike = class extends ConstructLike { static value: 208 = 208; };
const /*#classLike*/classLike = ClassLike.value;
const UnequalReturn = class extends UnequalConstructLike { static value: 209 = 209; };
const /*#unequalReturn*/unequalReturn = UnequalReturn.value;
const InheritedAbstract = class extends AbstractBase { static value: 210 = 210; };
const /*#inheritedAbstract*/inheritedAbstract = InheritedAbstract.value;
const ImplementedAbstract = class extends AbstractBase { static value: 211 = 211; member: number = 1; };
const /*#implementedAbstract*/implementedAbstract = ImplementedAbstract.value;
const PropertyToAccessor = class extends PropertyBase { static value: 212 = 212; get member(): number { return 1; } };
const /*#propertyToAccessor*/propertyToAccessor = PropertyToAccessor.value;
const AccessorToProperty = class extends AccessorBase { static value: 213 = 213; member: number = 1; };
const /*#accessorToProperty*/accessorToProperty = AccessorToProperty.value;
const MethodToAccessor = class extends MethodBase { static value: 214 = 214; get member(): () => number { return () => 1; } };
const /*#methodToAccessor*/methodToAccessor = MethodToAccessor.value;
const PrivateOverride = class extends PrivateMemberBase { static value: 215 = 215; private member: number = 1; };
const /*#privateOverride*/privateOverride = PrivateOverride.value;
const UninitializedOverride = class extends PropertyBase { static value: 216 = 216; member: number; };
const /*#uninitializedOverride*/uninitializedOverride = UninitializedOverride.value;
const DefiniteOverride = class extends PropertyBase { static value: 217 = 217; member!: number; };
const /*#definiteOverride*/definiteOverride = DefiniteOverride.value;
const InitializedOverride = class extends PropertyBase { static value: 218 = 218; member: number; constructor() { super(); this.member = 1; } };
const /*#initializedOverride*/initializedOverride = InitializedOverride.value;
const OverrideGood = class extends PropertyBase { static value: 219 = 219; override member: number = 1; };
const /*#overrideGood*/overrideGood = OverrideGood.value;
const OverrideMissing = class extends PropertyBase { static value: 220 = 220; override absent: number = 1; };
const /*#overrideMissing*/overrideMissing = OverrideMissing.value;
const OverrideSuggested = class extends PropertyBase { static value: 221 = 221; override membr: number = 1; };
const /*#overrideSuggested*/overrideSuggested = OverrideSuggested.value;
const OverrideStatic = class extends Plain { static value: 222 = 222; static override shared: string = ""; };
const /*#overrideStatic*/overrideStatic = OverrideStatic.value;
const OverrideParameter = class extends PropertyBase { static value: 223 = 223; constructor(public override member: number) { super(); } };
const /*#overrideParameter*/overrideParameter = OverrideParameter.value;
declare const dynamicKey: string;
const OverrideDynamic = class extends PropertyBase { static value: 224 = 224; override [dynamicKey]: number = 1; };
const /*#overrideDynamic*/overrideDynamic = OverrideDynamic.value;
const ImplementsGood = class implements Contract { static value: 225 = 225; member: number = 1; };
const /*#implementsGood*/implementsGood = ImplementsGood.value;
const ImplementsMismatch = class implements Contract { static value: 226 = 226; member: string = ""; };
const /*#implementsMismatch*/implementsMismatch = ImplementsMismatch.value;
const ImplementsAbsent = class implements Contract { static value: 227 = 227; };
const /*#implementsAbsent*/implementsAbsent = ImplementsAbsent.value;
const ImplementsInvalid = class implements string { static value: 228 = 228; };
const /*#implementsInvalid*/implementsInvalid = ImplementsInvalid.value;
const ImplementsMissing = class implements MissingContract { static value: 229 = 229; };
const /*#implementsMissing*/implementsMissing = ImplementsMissing.value;
const ImplementsGeneric = class implements GenericContract<number> { static value: 230 = 230; member: number = 1; };
const /*#implementsGeneric*/implementsGeneric = ImplementsGeneric.value;
const ImplementsBroken = class implements BrokenContract { static value: 231 = 231; member: number = 1; };
const /*#implementsBroken*/implementsBroken = ImplementsBroken.value;
function mixin<T extends new (...args: any[]) => {}>(Base: T) {
    const Mixed = class extends Base { static value: 232 = 232; };
    const /*#mixin*/mixin = Mixed.value;
    const InvalidMixed = class extends Base { static value: 233 = 233; constructor(value: number) { super(); } };
    const /*#invalidMixin*/invalidMixin = InvalidMixed.value;
}
function abstractMixin<T extends abstract new (...args: any[]) => {}>(Base: T) {
    const Mixed = class extends Base { static value: 234 = 234; };
    const /*#abstractMixin*/abstractMixin = Mixed.value;
}
