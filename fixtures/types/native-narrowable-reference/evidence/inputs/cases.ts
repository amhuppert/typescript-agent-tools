export function unconstrained<T>(value: T) { /*#unconstrained*/value; }
export function primitiveConstraint<T extends string>(value: T) { /*#primitiveConstraint*/value; }
export function unionStatement<T extends string | number>(value: T) { /*#unionStatement*/value; }
export function nullableStatement<T extends string | undefined>(value: T) { /*#nullableStatement*/value; }
export function primitiveContext<T extends string | number>(value: T) { const target: string | number = /*#primitiveContext*/value; }
export function nullableContext<T extends string | undefined>(value: T) { const target: string = /*#nullableContext*/value; }
export function genericContext<T extends string | number>(value: T) { const target: T = /*#genericContext*/value; }
export function nonGenericMissingContext(value: string) { const target: Missing = /*#nonGenericMissingContext*/value; }
export function primitiveMissingContext<T extends string>(value: T) { const target: Missing = /*#primitiveMissingContext*/value; }
export function missingConstraint<T extends Missing>(value: T) { /*#missingConstraint*/value; }
export function propertyReceiver<T extends { value: number } | { value: string }>(value: T) { /*#propertyReceiver*/value.value; }
export function elementReceiver<T extends { value: number } | { value: string }>(value: T) { /*#elementReceiver*/value["value"]; }
export function genericElementReceiver<T extends { value: number }, K extends "value">(value: T, key: K) { /*#genericElementReceiver*/value[key]; }
export function callReceiver<T extends (() => number) | (() => string)>(value: T) { /*#callReceiver*/value(); }
export function newReceiver<T extends (new () => { value: number }) | (new () => { value: string })>(value: T) { new /*#newReceiver*/value(); }
export function parenthesizedContext<T extends string | number>(value: T) { const target: string | number = (/*#parenthesizedContext*/value); }
export function satisfiesContext<T extends string | number>(value: T) { (/*#satisfiesContext*/value satisfies string | number); }
export function typeofOperand<T extends string | number>(value: T) { typeof /*#typeofOperand*/value; }
export function uninitializedContext<T extends string | number>(value: T) { const target = /*#uninitializedContext*/value; }
export function unionOfParameter<T extends string | undefined>(value: T | number) { const target: string | number | undefined = /*#unionOfParameter*/value; }
