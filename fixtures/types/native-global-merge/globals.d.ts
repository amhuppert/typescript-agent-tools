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
interface Box<T> { first: T; firstNested: { value: T } }
declare function pick(value: number): "first-number";
declare enum Color { Red = 1 }
