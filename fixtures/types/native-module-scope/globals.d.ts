interface Array<T> { readonly length: number; [n: number]: T; }
interface Boolean {}
interface Function {}
interface CallableFunction extends Function {}
interface NewableFunction extends Function {}
interface IArguments {}
interface Number {}
interface Object {}
interface RegExp {}
interface String {}
declare const ReexportValue: 71;
interface ReexportType { value: 72; }
declare namespace ReexportNamespace { export const value: 73; }
declare const StarValue: 74;
interface Mixed { value: 99; }
declare const Mixed: 79;
interface Lowercase { value: 75; }
declare const AmbientReexport: 76;
declare namespace GlobalTarget { export type Item = 77; }
declare module "scope-dependency" { export const remoteValue: 88; }
