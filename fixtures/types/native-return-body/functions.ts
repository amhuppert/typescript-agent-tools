export function scalar() { return 1; }
export function stringValue() { return "ready"; }
export function booleanPair(flag: boolean) { if (flag) return true; return false; }
export function objectValue() { return { text: "ready", count: 1 }; }
export function noReturn() { let unused = 1; }
export function bareReturn() { return; }
export function implicitReturn(flag: boolean) { if (flag) return 1; }
export function explicitBareReturn(flag: boolean) { if (flag) return 1; return; }
export function allReturns(flag: boolean) { if (flag) return 1; return 2; }
export function mixedReturns(flag: boolean) { if (flag) return 1; return "ready"; }
export function nestedReturns() { function inner() { return "inner"; } return 1; }
export function unreachableReturn() { return 1; return "unreachable"; }
export function unreachableBareReturn() { return 1; return; }
export function throwingDeclaration() { throw 1; }
export function infiniteDeclaration() { while (true) {} }
export function annotated(): number { return Missing; }
export function invalidReturn() { return Missing; }
export function recursiveOnly() { return recursiveOnly(); }
export function recursiveAndScalar(flag: boolean) { if (flag) return recursiveAndScalar(flag); return 1; }
declare const neverValue: never;
export function explicitNeverValue() { return neverValue; }
export const throwingExpression = function () { throw 1; };
export const throwingArrow = () => { throw 1; };
export const scalarArrow = () => 1;
export const constAssertion = () => ("ready" as const);
export const contextualUndefined: () => undefined = () => {};
export const contextualLiteral: () => "ready" = () => "ready";
export async function asyncScalar() { return 1; }
export function* generatorScalar() { yield "ready"; return 1; }
