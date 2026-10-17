export {};
function unrelatedStrict(x: number) { if (1 === 1) { return /*#unrelatedStrict*/x; } }
function unrelatedLoose(x: number) { if (1 == 1) { return /*#unrelatedLoose*/x; } }
function unrelatedNotStrict(x: number) { if (1 !== 2) { return /*#unrelatedNotStrict*/x; } }
function unrelatedNotLoose(x: number) { if (1 != 2) { return /*#unrelatedNotLoose*/x; } }
function selfStrict(x: number) { if (x === x) { return /*#selfStrict*/x; } }
function selfLoose(x: number) { if (x == x) { return /*#selfLoose*/x; } }
function selfNotStrict(x: number) { if (x !== x) { return /*#selfNotStrict*/x; } }
function selfNotLoose(x: number) { if (x != x) { return /*#selfNotLoose*/x; } }
function booleanRight(x: number) { if ((1 === 1) === true) { return /*#booleanRight*/x; } }
function booleanLeft(x: number) { if (false !== (1 === 1)) { return /*#booleanLeft*/x; } }
function typeofSame(x: number) { if (typeof x === "number") { return /*#typeofSame*/x; } }
function typeofSwapped(x: number) { if ("number" == typeof x) { return /*#typeofSwapped*/x; } }
function typeofOther(x: number, y: string) { if (typeof y === "string") { return /*#typeofOther*/x; } }
function anySkips(x: any, y: MissingSkipped) { if (x === y) { return /*#anySkips*/x; } }
function anySwappedSkips(x: any, y: MissingSkippedSwapped) { if (y === x) { return /*#anySwappedSkips*/x; } }
function matchedMissing(x: number, y: MissingConsumed) { if (x === y) { return /*#matchedMissing*/x; } }
function literalChanges(x: number) { if (x === 1) { return /*#literalChanges*/x; } }
function swappedChanges(x: number) { if (1 == x) { return /*#swappedChanges*/x; } }
function negativeChanges(x: 1 | 2) { if (x !== 1) { return /*#negativeChanges*/x; } }
function nullableChanges(x: number | undefined) { if (x !== undefined) { return /*#nullableChanges*/x; } }
function nullableLoose(x: number) { if (x == null) { return /*#nullableLoose*/x; } }
function optionalSame(x: { value: number } | undefined) { if (x?.value === undefined) { return /*#optionalSame*/x; } }
function optionalChanges(x: { value: number } | undefined) { if (x?.value === 1) { return /*#optionalChanges*/x; } }
function typeofChanges(x: number | string) { if (typeof x === "number") { return /*#typeofChanges*/x; } }
type Choice = { kind: "a"; value: number } | { kind: "b"; value: number };
function discriminantChanges(x: Choice) { if (x.kind === "a") { return /*#discriminantChanges*/x; } }
interface HasConstructor { constructor: unknown }
function constructorInequality(x: HasConstructor, y: MissingConstructor) { if (x.constructor !== y) { return /*#constructorInequality*/x; } }
declare const NonConstructor: number;
function constructorNumber(x: HasConstructor) { if (x.constructor === NonConstructor) { return /*#constructorNumber*/x; } }
