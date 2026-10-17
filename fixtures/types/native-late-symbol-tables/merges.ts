export {};
declare const propertyKey: "value";
declare const methodKey: "method";
interface LateProperties { [propertyKey]: 21; }
interface LateProperties { [propertyKey]: 22; }
declare const lateProperties: LateProperties;
const /*#latePropertyFirstValue*/latePropertyFirstValue = lateProperties.value;

interface EarlyAndLate { value: 23; [propertyKey]: 24; }
declare const earlyAndLate: EarlyAndLate;
const /*#earlyPropertyFirstValue*/earlyPropertyFirstValue = earlyAndLate.value;

interface LateMethods { [methodKey](value: number): "number"; }
interface LateMethods { [methodKey](value: string): "string"; }
declare const lateMethods: LateMethods;
const /*#lateMethodNumber*/lateMethodNumber = lateMethods.method(1);
const /*#lateMethodString*/lateMethodString = lateMethods.method("x");

declare class LateAccessors {
    get [propertyKey](): number;
    set [propertyKey](value: number | string);
}
declare const lateAccessors: LateAccessors;
const /*#lateAccessorRead*/lateAccessorRead = lateAccessors.value;
const /*#lateAccessorWrite*/lateAccessorWrite = lateAccessors.value = "accepted";
const /*#lateAccessorRejectedWrite*/lateAccessorRejectedWrite = lateAccessors.value = true;

declare class EarlyLateAccessors {
    get value(): number;
    set [propertyKey](value: number | string);
}
declare const earlyLateAccessors: EarlyLateAccessors;
const /*#earlyLateAccessorRead*/earlyLateAccessorRead = earlyLateAccessors.value;
const /*#earlyLateAccessorWrite*/earlyLateAccessorWrite = earlyLateAccessors.value = "accepted";

declare class DuplicateGetter {
    get [propertyKey](): 25;
    get [propertyKey](): 26;
}
declare const duplicateGetterValue: DuplicateGetter;
const /*#duplicateGetter*/duplicateGetter = duplicateGetterValue.value;

interface Conflict { [propertyKey]: 27; [propertyKey](): 28; }
declare const conflict: Conflict;
const /*#latePropertyMethodConflict*/latePropertyMethodConflict = conflict.value;
