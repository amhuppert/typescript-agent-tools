interface Object {}
interface Function {}
interface CallableFunction extends Function {}
interface NewableFunction extends Function {}
interface String {}
interface Number {}
interface Boolean {}
interface RegExp {}
interface IArguments { length: number; [index: number]: any; }
interface Symbol {}
interface SymbolConstructor { readonly iterator: unique symbol; }
declare const Symbol: SymbolConstructor;
interface IteratorYieldResult<TYield> { done?: false; value: TYield; }
interface IteratorReturnResult<TReturn> { done: true; value: TReturn; }
type IteratorResult<T, TReturn = any> = IteratorYieldResult<T> | IteratorReturnResult<TReturn>;
interface Iterator<T, TReturn = any, TNext = any> { next(...args: [] | [TNext]): IteratorResult<T, TReturn>; }
interface Iterable<T> { [Symbol.iterator](): Iterator<T>; }
interface Array<T> extends Iterable<T> { length: number; [index: number]: T; push(...items: T[]): number; }
interface ReadonlyArray<T> extends Iterable<T> { readonly length: number; readonly [index: number]: T; }
