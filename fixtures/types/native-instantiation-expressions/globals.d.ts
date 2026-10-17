interface Object {}
interface Function {}
interface CallableFunction {}
interface NewableFunction {}
interface String { readonly length: number; }
interface Number {}
interface Boolean {}
interface BigInt {}
interface Symbol {}
interface Array<T> { readonly length: number; [n: number]: T; }
interface ReadonlyArray<T> { readonly length: number; readonly [n: number]: T; }
