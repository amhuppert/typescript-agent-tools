interface Object {}
interface Function {}
interface CallableFunction {}
interface NewableFunction {}
interface String { length: number; charAt(pos: number): string; }
interface Number { toFixed(digits?: number): string; }
interface Boolean { valueOf(): boolean; }
interface BigInt {}
interface Symbol {}
interface Array<T> { readonly length: number; [n: number]: T; }
interface ReadonlyArray<T> { readonly length: number; readonly [n: number]: T; }
