interface Object {}
interface Function { apply(thisArg: any, argArray?: any): any }
interface CallableFunction {}
interface NewableFunction {}
interface String {}
interface Number {}
interface Boolean {}
interface RegExp {}
interface IArguments { length: number; [index: number]: any }
declare const Symbol: { readonly iterator: "iter" };
interface IteratorYieldResult<T> { done?: false; value: T }
interface IteratorReturnResult<T> { done: true; value: T }
type IteratorResult<T, TReturn> = IteratorYieldResult<T> | IteratorReturnResult<TReturn>;
interface Iterator<T, TReturn = any, TNext = any> { next(...args: [] | [TNext]): IteratorResult<T, TReturn> }
interface Iterable<T, TReturn = any, TNext = any> { iter(): Iterator<T, TReturn, TNext> }
interface Array<T> { length: number; [index: number]: T; iter(): Iterator<T, undefined, unknown> }
interface ReadonlyArray<T> { readonly length: number; readonly [index: number]: T; iter(): Iterator<T, undefined, unknown> }
