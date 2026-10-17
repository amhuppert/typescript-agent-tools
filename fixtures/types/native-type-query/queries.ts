import type { fixed as typeFixed, objectValue as typeObject } from "./exports";
import { count as importedCount, choice as importedChoice } from "./exports";
export {};
const fixed = 1;
let count = 1;
const objectValue = { text: "ready", count: 1 };
type Fixed = /*fixed-type*/ typeof /*fixed-value*/ fixed;
type Count = /*count-type*/ typeof /*count-value*/ count;
type ObjectValue = /*object-type*/ typeof /*object-value*/ objectValue;
type TypeFixed = /*type-import-type*/ typeof /*type-import-value*/ typeFixed;
type TypeObject = /*type-object-type*/ typeof /*type-object-value*/ typeObject;
type ImportedCount = /*import-type*/ typeof /*import-value*/ importedCount;
function nonautomatic(): void {
  let pending: number;
  type Pending = /*pending-type*/ typeof /*pending-value*/ pending;
  /*ordinary-pending*/ pending;
}
function automatic(): void {
  let inferred;
  type Inferred = /*automatic-type*/ typeof /*automatic-value*/ inferred;
}
let narrowed: string | number = 1;
type Narrowed = /*narrowed-type*/ typeof /*narrowed-value*/ narrowed;
type Choice = /*choice-type*/ typeof /*choice-value*/ importedChoice;
type Missing = /*missing-type*/ typeof /*missing-value*/ absentValue;
function generic<T>(value: T): T { return value; }
type Instantiated = /*instantiated-type*/ typeof /*instantiated-value*/ generic<string>;
declare namespace Space { const count: number; }
type Qualified = /*qualified-type*/ typeof Space./*qualified-value*/count;
/*runtime-type-import*/ typeFixed;
/*runtime-type-object*/ typeObject;
type ArgumentOnly = typeof generic</*argument-type*/MissingArgument>;
import { type fixed as specifierFixed, type objectValue as specifierObject } from "./exports";
import { restrictedFixed as exportedFixed, restrictedObject as exportedObject } from "./barrel";
type SpecifierFixed = /*specifier-type*/ typeof /*specifier-value*/ specifierFixed;
type SpecifierObject = /*specifier-object-type*/ typeof /*specifier-object-value*/ specifierObject;
type ExportedFixed = /*exported-type*/ typeof /*exported-value*/ exportedFixed;
type ExportedObject = /*exported-object-type*/ typeof /*exported-object-value*/ exportedObject;
/*runtime-specifier*/ specifierFixed;
/*runtime-specifier-object*/ specifierObject;
/*runtime-exported*/ exportedFixed;
/*runtime-exported-object*/ exportedObject;
class Holder { field = 1; method(): void { type Self = /*this-type*/ typeof /*this-value*/ this; } }
