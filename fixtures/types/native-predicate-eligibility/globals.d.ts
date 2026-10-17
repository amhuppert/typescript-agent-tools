interface Object {}
interface Function {}
interface CallableFunction extends Function {}
interface NewableFunction extends Function {}
interface IArguments { length: number; [index: number]: any; }
interface String {}
interface Number {}
interface Boolean {}
interface RegExp {}
interface Array<T> { length: number; [index: number]: T; }
interface ReadonlyArray<T> { readonly length: number; readonly [index: number]: T; }
