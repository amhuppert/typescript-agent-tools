export {};
declare const textKey: "text";
declare const numberKey: 17;
declare const uniqueKey: unique symbol;
declare const nested: { readonly key: "nested" };
interface Names {
    fixed: 0;
    [textKey]: 1;
    [numberKey]: 2;
    [uniqueKey]: 3;
    [nested.key]: 4;
    [Symbol.iterator](): 5;
    ["literal"]: 6;
    [-1]: 7;
}
declare const names: Names;
const /*#earlyName*/earlyName = names.fixed;
const /*#stringName*/stringName = names.text;
const /*#numberName*/numberName = names[17];
const /*#uniqueName*/uniqueName = names[uniqueKey];
const /*#propertyEntityName*/propertyEntityName = names.nested;
const /*#iteratorName*/iteratorName = names[Symbol.iterator]();
const /*#literalEarlyName*/literalEarlyName = names.literal;
const /*#signedEarlyName*/signedEarlyName = names[-1];

declare const classKey: "side";
declare const absentInstanceKey: MissingInstanceKey;
declare const absentStaticKey: MissingStaticKey;
declare class StaticOnly {
    static [classKey]: 10;
    [absentInstanceKey]: MissingInstanceValue;
}
declare class InstanceOnly {
    [classKey]: 11;
    static [absentStaticKey]: MissingStaticValue;
}
declare const instanceOnly: InstanceOnly;
const /*#staticPartition*/staticPartition = StaticOnly.side;
const /*#instancePartition*/instancePartition = instanceOnly.side;

declare function keyCall(): "call";
interface NonEntity {
    stable: 12;
    [keyCall()]: MissingCallValue;
    [nested["key"]]: MissingElementValue;
    [(textKey)]: MissingParenthesizedValue;
}
declare const nonEntity: NonEntity;
const /*#nonEntityUntouched*/nonEntityUntouched = nonEntity.stable;
const /*#nonEntityCallAbsent*/nonEntityCallAbsent = nonEntity.call;
const /*#nonEntityElementAbsent*/nonEntityElementAbsent = nonEntity.nested;
const /*#nonEntityParenAbsent*/nonEntityParenAbsent = nonEntity.text;

type LiteralBody = { [textKey]: 13 };
declare const literalBody: LiteralBody;
const /*#typeLiteralName*/typeLiteralName = literalBody.text;
const objectBody = { [textKey]: 14 as const };
const /*#objectLiteralName*/objectLiteralName = objectBody.text;
