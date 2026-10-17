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
interface Boxdeepsmall<T> { value: T }
declare function deepdeepsmall<T>(): Boxdeepsmall<Boxdeepsmall<Boxdeepsmall<Boxdeepsmall<Boxdeepsmall<Boxdeepsmall<Boxdeepsmall<Boxdeepsmall<T>>>>>>>>;
function querydeepsmall() {
  const deepChosen = (deepdeepsmall<number>(), 0 as number);
  const healthy = 0 as number;
}
