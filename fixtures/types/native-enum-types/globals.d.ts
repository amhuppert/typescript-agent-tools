interface Array<T> { length: number; [index: number]: T }
interface ReadonlyArray<T> { readonly length: number; readonly [index: number]: T }
interface Object {}
interface Function { apply(thisArg: any, argArray?: any): any }
interface CallableFunction {}
interface NewableFunction {}
interface IArguments { length: number; [index: number]: any }
interface String {}
interface Number {}
interface Boolean {}
interface RegExp {}
declare const Infinity: number;
declare const NaN: number;
