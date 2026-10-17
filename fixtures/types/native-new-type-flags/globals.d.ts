interface Object {}
interface Function {}
interface CallableFunction extends Function {}
interface NewableFunction extends Function {}
interface String {}
interface Number {}
interface Boolean {}
interface RegExp {}
interface IArguments { length: number; [index: number]: any; }
interface Array<T> { length: number; [index: number]: T; }
type NoInfer<T> = intrinsic;
