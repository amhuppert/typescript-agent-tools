interface Box { p: number; q: string; optional?: boolean; broken: Missing; }
export declare const plain: Box["p"];
export declare const optional: Box["optional"];
export declare const union: Box["p" | "q"];
export declare const absent: Box["absent"];
export declare const consumed: Box["broken"];
export declare const unused: { good: number; bad: Missing }["good"];
export declare const indexed: { [key: string]: number }[string];
export declare const numbered: { [key: number]: string }[number];
export declare const neverKey: Box[never];
export declare const anyObject: any["p"];
export declare const neverObject: never["p"];
export declare const unknownObject: unknown["p"];
export declare const invalidKey: Box[boolean];
export declare const indexedUnion: ({ [key: string]: number } | { [key: string]: string })[string];
export declare const indexedIntersection: ({ [key: string]: number } & { [key: string]: string })[string];
export declare const readonlyIndex: { readonly [key: string]: number }[string];
type Lookup<T extends {p: number}> = T["p"];
export declare const instantiated: Lookup<{p: 42}>;
export function generic<T extends {p: number}>(parameter: T["p"], object: T) {
  /*#generic*/parameter;
  /*#expression*/object["p"];
}
export function genericKey<K extends "p">(object: Box, key: K) {
  /*#genericKey*/object[key];
}
export function unionGeneric<T extends Box>(parameter: T["p" | "q"]) {
  /*#unionGeneric*/parameter;
}
export function constrainedIndex<K extends "p" | "q">(parameter: Box[K]) {
  /*#constrainedIndex*/parameter;
}
declare const unaryMixed: number | bigint;
declare const unaryBig: bigint;
declare const unaryNumber: number;
const /*#negativeMixed*/negativeMixed = -unaryMixed;
const /*#tildeMixed*/tildeMixed = ~unaryMixed;
const /*#negativeBig*/negativeBig = -unaryBig;
const /*#tildeBig*/tildeBig = ~unaryBig;
const /*#negativeNumber*/negativeNumber = -unaryNumber;
const /*#tildeNumber*/tildeNumber = ~unaryNumber;
export function unaryGeneric<T extends {p: bigint}>(value: T["p"]) {
  const /*#negativeGeneric*/negativeGeneric = -value;
  const /*#tildeGeneric*/tildeGeneric = ~value;
}
