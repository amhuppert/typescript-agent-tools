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
declare function flatflatsmall<T>(x: "k0" | "k1" | "k2" | "k3" | "k4" | "k5" | "k6" | "k7" | "k8" | "k9" | "k10" | "k11" | "k12" | "k13" | "k14" | "k15" | T): T;
function queryflatsmall() {
  const flatChosen = (flatflatsmall<number>(1), 0 as number);
  const healthy = 0 as number;
}
