interface Array<T> { [index: number]: T }
interface ReadonlyArray<T> { readonly [index: number]: T }
interface String { /*#lengthDeclaration*/length: number }
declare const Object: { defineProperty(target: object, key: string, descriptor: object): object };
declare module /*#ambientDeclaration*/"ambient" { export const item: number; }
