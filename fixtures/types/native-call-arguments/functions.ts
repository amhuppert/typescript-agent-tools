export function numberParam(value: number): number { return 1; }
export function stringParam(value: string): string { return "done"; }
export function literalParam(value: "ready"): "ready" { return "ready"; }
export function booleanParam(value: boolean): boolean { return true; }
export function unionParam(value: number | string): number { return 1; }
export function anyParam(value: any): number { return 1; }
export function unknownParam(value: unknown): number { return 1; }
export function neverParam(value: never): number { return 1; }
export function voidParam(value: void): number { return 1; }
export function optionalParam(value?: string): number { return 1; }
export function withThis(this: number, value: number): number { return 1; }
export function twoNumbers(first: number, second: number): number { return 1; }
declare const untyped: any;
export function /*#number*/checkedNumber() { return numberParam(1); }
export function /*#string*/checkedString() { return stringParam("value"); }
export function /*#literal*/checkedLiteral() { return literalParam("ready"); }
export function /*#boolean*/checkedBoolean() { return booleanParam(true); }
export function /*#unionNumber*/checkedUnionNumber() { return unionParam(1); }
export function /*#unionString*/checkedUnionString() { return unionParam("value"); }
export function /*#any*/checkedAny() { return anyParam("value"); }
export function /*#unknown*/checkedUnknown() { return unknownParam("value"); }
export function /*#optional*/checkedOptional() { return optionalParam("value"); }
export function /*#void*/checkedVoid() { return voidParam(void 0); }
export function /*#wrongPrimitive*/checkedWrongPrimitive() { return numberParam("bad"); }
export function /*#wrongLiteral*/checkedWrongLiteral() { return literalParam("bad"); }
export function /*#never*/checkedNever() { return neverParam(1); }
export function /*#null*/checkedNull() { return numberParam(null); }
export function /*#thisBeforeMissing*/checkedThisBeforeMissing() { return withThis(Missing); }
export function /*#wrongBeforeMissing*/checkedWrongBeforeMissing() { return twoNumbers("bad", Missing); }
export function /*#missing*/checkedMissing() { return numberParam(Missing); }
export function /*#anyMissing*/checkedAnyMissing() { return untyped(Missing); }

declare const mixed: number | string;
declare const exact: "ready" | "other";
export function /*#unionSource*/checkedUnionSource() { return unionParam(mixed); }
export function /*#unionRejected*/checkedUnionRejected() { return stringParam(mixed); }
export function /*#literalUnion*/checkedLiteralUnion() { return stringParam(exact); }

export function numericLiteralParam(value: 100): 100 { return 100; }
export function bigintParam(value: bigint): bigint { return 10n; }
export function bigintLiteralParam(value: 10n): 10n { return 10n; }
export function falseLiteralParam(value: false): false { return false; }
export function symbolParam(value: symbol): symbol { return value; }
declare const token: symbol;
export function /*#numericEquivalent*/checkedNumericEquivalent() { return numericLiteralParam(1e2); }
export function /*#numericRejected*/checkedNumericRejected() { return numericLiteralParam(101); }
export function /*#bigint*/checkedBigint() { return bigintParam(10n); }
export function /*#bigintEquivalent*/checkedBigintEquivalent() { return bigintLiteralParam(0xan); }
export function /*#bigintRejected*/checkedBigintRejected() { return bigintLiteralParam(11n); }
export function /*#false*/checkedFalse() { return falseLiteralParam(false); }
export function /*#booleanRejected*/checkedBooleanRejected() { return falseLiteralParam(true); }
export function /*#symbol*/checkedSymbol() { return symbolParam(token); }
export function /*#escaped*/checkedEscaped() { return literalParam("\u0072eady"); }
export function /*#optionalUndefined*/checkedOptionalUndefined() { return optionalParam(void 0); }
