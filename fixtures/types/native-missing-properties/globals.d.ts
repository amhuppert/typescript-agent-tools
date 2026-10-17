interface Object {}
interface Function {}
interface CallableFunction {}
interface NewableFunction {}
interface String {}
interface Number {}
interface Boolean {}
interface BigInt {}
interface Symbol {}
interface Array<T> { readonly length: number; [n: number]: T; }
interface ReadonlyArray<T> { readonly length: number; readonly [n: number]: T; }
interface IArguments {}
interface RegExp {}
interface Promise<T> { then<TResult>(onfulfilled: (value: T) => TResult): Promise<TResult>; }
interface Date {}
