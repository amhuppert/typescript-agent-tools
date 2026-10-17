interface Object {}
interface Function {}
interface Array<T> { length: number; [n: number]: T }
interface ReadonlyArray<T> { readonly length: number; readonly [n: number]: T }
interface String { readonly length: number }
interface Number {}
interface Boolean {}
interface RegExp {}
interface IArguments {}
interface CallableFunction extends Function {}
interface NewableFunction extends Function {}

declare const key: unique symbol;

declare function stringIndex(input: { [name: string]: number }): 1;
declare function stringIndex(input: { [key]: number }): 2;
const /*#stringIndexUnique*/stringIndexUnique = stringIndex({ [key]: 10 });

declare function symbolIndex(input: { [name: symbol]: number }): 3;
declare function symbolIndex(input: { [key]: number }): 4;
const /*#symbolIndexUnique*/symbolIndexUnique = symbolIndex({ [key]: 10 });

declare function numberIndex(input: { [name: number]: number }): 5;
declare function numberIndex(input: { [key]: number }): 6;
const /*#numberIndexUnique*/numberIndexUnique = numberIndex({ [key]: 10 });

declare function exactSymbol(input: { [key]: number }): 7;
declare function exactSymbol(input: { [name: string]: number }): 8;
const /*#exactUnique*/exactUnique = exactSymbol({ [key]: 10 });

declare function unionString(input: { plain: boolean } | { [name: string]: number }): 9;
declare function unionString(input: { [key]: number }): 10;
const /*#unionStringUnique*/unionStringUnique = unionString({ [key]: 10 });

declare function intersectionString(input: { optional?: boolean } & { [name: string]: number }): 11;
declare function intersectionString(input: { [key]: number }): 12;
const /*#intersectionStringUnique*/intersectionStringUnique = intersectionString({ [key]: 10 });

declare function plainString(input: { [name: string]: number }): 13;
declare function plainString(input: { plain: number }): 14;
const /*#stringIndexPlain*/stringIndexPlain = plainString({ plain: 10 });

declare function numeric(input: { [name: number]: number }): 15;
declare function numeric(input: { 0: number }): 16;
const /*#numberIndexNumeric*/numberIndexNumeric = numeric({ 0: 10 });

declare function differentValue(input: { [name: string]: number }): 17;
declare function differentValue(input: { [key]: string }): 18;
const /*#stringIndexDifferentValue*/stringIndexDifferentValue = differentValue({ [key]: "text" });

declare function noIndex(input: { optional?: boolean }): 19;
declare function noIndex(input: { [key]: number }): 20;
const /*#noIndexUnique*/noIndexUnique = noIndex({ [key]: 10 });

declare function methodIndex(input: { [name: string]: () => number }): 21;
declare function methodIndex(input: { [key](): number }): 22;
const /*#stringIndexMethod*/stringIndexMethod = methodIndex({ [key]() { return 10; } });

declare const strings: { [name: string]: number };
declare const symbols: { [name: symbol]: number };
declare const numbers: { [name: number]: number };
const /*#stringElementUnique*/stringElementUnique = strings[key];
const /*#symbolElementUnique*/symbolElementUnique = symbols[key];
const /*#numberElementUnique*/numberElementUnique = numbers[key];
const /*#stringElementPlain*/stringElementPlain = strings["plain"];
