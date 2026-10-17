interface Shape { z: number; a?: string; 2: boolean; broken: Missing; }
type Alias = { later: number; first: string; };
class Example { public visible: number; protected hidden: number; private secret: number; 3: boolean; method(): void {} }
export declare const shape: keyof Shape;
export declare const alias: keyof Alias;
export declare const literal: keyof { z: number; a: string; 4: boolean; };
export declare const classKeys: keyof Example;
export declare const stringIndex: keyof { [key: string]: number; named: number; };
export declare const numberIndex: keyof { [key: number]: string; named: number; };
export declare const brokenIndex: keyof { [key: string]: Missing; };
export declare const union: keyof ({ common: number; left: string; } | { common: string; right: number; });
export declare const intersection: keyof ({ left: number } & { right: string });
export declare const anyKeys: keyof any;
export declare const neverKeys: keyof never;
export declare const unknownKeys: keyof unknown;
export declare const objectKeys: keyof object;
export declare const emptyKeys: keyof {};
export declare const stringKeys: keyof string;
export declare const numberKeys: keyof number;
export declare const booleanKeys: keyof boolean;
export declare const nullKeys: keyof null;
export declare const undefinedKeys: keyof undefined;
export declare const tupleKeys: keyof [first: number, second?: string];
type Keys<T> = keyof T;
export declare const instantiated: Keys<{ b: number; a: string; }>;
export function generic<T>(parameter: keyof T) { /*#generic*/parameter; }
export function genericIntersection<T>(parameter: keyof (T & {})) { /*#genericIntersection*/parameter; }
export function genericUnion<T>(parameter: keyof (T | {})) { /*#genericUnion*/parameter; }
export declare const invalid: keyof Missing;
