namespace Left { export enum E { A = 1, B = 2 } }
namespace Equal { export enum E { A = 1, B = 2 } }
namespace Extra { export enum E { A = 1, B = 2, C = 3 } }
namespace Missing { export enum E { A = 1 } }
namespace Different { export enum E { A = 1, B = 3 } }
namespace OtherName { export enum Other { A = 1, B = 2 } }
namespace StringLeft { export enum E { A = "a", B = "b" } }
namespace StringEqual { export enum E { A = "a", B = "b" } }
namespace StringDifferent { export enum E { A = "a", B = "c" } }
namespace Opaque { export declare enum E { A, B } }
namespace OpaqueEqual { export declare enum E { A, B } }
namespace ConstLeft { export const enum E { A = 1, B = 2 } }
namespace ConstEqual { export const enum E { A = 1, B = 2 } }
declare let left: Left.E, equal: Equal.E, extra: Extra.E, missing: Missing.E, different: Different.E, other: OtherName.Other;
declare let strings: StringLeft.E, stringsEqual: StringEqual.E, stringsDifferent: StringDifferent.E;
declare let opaque: Opaque.E, opaqueEqual: OpaqueEqual.E, constant: ConstLeft.E, constantEqual: ConstEqual.E;
declare function numeric(x: Left.E): "match";
declare function numeric(x: unknown): "fallback";
declare function stringEnum(x: StringLeft.E): "match";
declare function stringEnum(x: unknown): "fallback";
declare function opaqueEnum(x: Opaque.E): "match";
declare function opaqueEnum(x: unknown): "fallback";
declare function constEnum(x: ConstLeft.E): "match";
declare function constEnum(x: unknown): "fallback";
const /*#same*/same = numeric(left);
const /*#sameNameEqual*/sameNameEqual = numeric(equal);
const /*#sourceExtra*/sourceExtra = numeric(extra);
const /*#sourceMissing*/sourceMissing = numeric(missing);
const /*#differentValue*/differentValue = numeric(different);
const /*#differentName*/differentName = numeric(other);
const /*#stringEqual*/stringEqual = stringEnum(stringsEqual);
const /*#stringDifferent*/stringDifferent = stringEnum(stringsDifferent);
const /*#opaqueEqual*/opaqueResult = opaqueEnum(opaqueEqual);
const /*#knownToOpaque*/knownToOpaque = opaqueEnum(left);
const /*#opaqueToKnown*/opaqueToKnown = numeric(opaque);
const /*#stringToOpaque*/stringToOpaque = opaqueEnum(strings);
const /*#opaqueToString*/opaqueToString = stringEnum(opaque);
const /*#constSame*/constSame = constEnum(constant);
const /*#constDifferent*/constDifferent = constEnum(constantEqual);
const /*#cachedSuccess*/cachedSuccess = (numeric(equal), numeric(equal));
const /*#cachedFailure*/cachedFailure = (numeric(different), numeric(different));
declare function strictNumeric(x: Left.E): "match";
declare function strictString(x: StringLeft.E): "match";
const /*#reportValues*/reportValues = strictNumeric(different);
const /*#reportMissing*/reportMissing = strictNumeric(extra);
const /*#reportOpaque*/reportOpaque = strictString(opaque);
const /*#reportTwice*/reportTwice = (strictNumeric(different), strictNumeric(different));
type /*#enumKeys*/EnumKeys = keyof typeof Left.E;
const /*#ownReverse*/ownReverse = Left.E[Left.E.A];
const /*#otherReverse*/otherReverse = Left.E[Equal.E.A];
const /*#numberReverse*/numberReverse = Left.E[5];
const /*#constString*/constString = ConstLeft.E["A"];
const /*#constNumber*/constNumber = ConstLeft.E[1];
const /*#constDynamicString*/constDynamicString = ConstLeft.E["A" as "A"];
const /*#qualifiedConst*/qualifiedConst = Left.E.A as const;
type /*#mappedEnum*/MappedEnum = { [K in Opaque.E]: string };
type /*#numericTemplate*/NumericTemplate = `${Left.E}`;
type /*#stringTemplate*/StringTemplate = `${StringLeft.E}`;

// Comma initializers enter Normal checking, while the earlier single-call
// declarations preserve getQuickTypeOfExpression's skipped applicability.
const /*#normalValues*/normalValues = (strictNumeric(different), 0);
const /*#normalMissing*/normalMissing = (strictNumeric(extra), 0);
const /*#normalOpaque*/normalOpaque = (strictString(opaque), 0);
const /*#normalMissingTwice*/normalMissingTwice = (strictNumeric(extra), strictNumeric(extra), 0);
const /*#normalOpaqueTwice*/normalOpaqueTwice = (strictString(opaque), strictString(opaque), 0);

declare function computedNumber(): number;
enum Computed { Value = computedNumber() }
enum OtherComputed { Value = computedNumber() }
class FreshComputedProperty { readonly value = Computed.Value; }
type RegularComputedProperty = { readonly value: Computed.Value };
type Identical<X, Y> = (<T>() => T extends X ? 1 : 2) extends (<T>() => T extends Y ? 1 : 2) ? "same" : "different";
type /*#computedFreshIdentity*/ComputedFreshIdentity = Identical<FreshComputedProperty, RegularComputedProperty>;
type /*#computedReverseIdentity*/ComputedReverseIdentity = Identical<RegularComputedProperty, FreshComputedProperty>;
type /*#computedOtherIdentity*/ComputedOtherIdentity = Identical<FreshComputedProperty, { readonly value: OtherComputed.Value }>;
declare function acceptComputed(value: Computed.Value): "computed";
declare function acceptComputed(value: unknown): "fallback";
const /*#computedFreshArgument*/computedFreshArgument = acceptComputed(Computed.Value);
const /*#computedRegularArgument*/computedRegularArgument = acceptComputed(null as unknown as Computed.Value);
const /*#computedOtherArgument*/computedOtherArgument = acceptComputed(OtherComputed.Value);

// Both names use the same Value resolver; enum auto-values use their name
// text in the flow reference key operation, not getEnumMemberValue.
namespace Keys { export const explicit = "A"; export const annotated: "B" = "B"; }
const /*#qualifiedVariableKey*/qualifiedVariableKey = Left.E[Keys.explicit];
const /*#qualifiedAnnotatedKey*/qualifiedAnnotatedKey = Left.E[Keys.annotated];
enum AutomaticKey { A }
const /*#automaticEnumKey*/automaticEnumKey = AutomaticKey[AutomaticKey.A];
