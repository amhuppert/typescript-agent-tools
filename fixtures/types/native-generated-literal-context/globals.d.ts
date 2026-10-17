interface Object {}
interface Function {}
interface CallableFunction extends Function {}
interface NewableFunction extends Function {}
interface String {}
interface Number {}
interface Boolean {}
interface RegExp {}
interface IArguments { length: number; [index: number]: any; }
interface Array<T> { length: number; [index: number]: T; push(...items: T[]): number; }
interface ReadonlyArray<T> { readonly length: number; readonly [index: number]: T; }

interface Promise<T> { then(onfulfilled: (value: T) => unknown): unknown; }
interface PromiseLike<T> { then(onfulfilled: (value: T) => unknown): unknown; }
declare const Promise: any;

type Uppercase<S extends string> = intrinsic;
type Lowercase<S extends string> = intrinsic;
type Capitalize<S extends string> = intrinsic;
type Uncapitalize<S extends string> = intrinsic;
