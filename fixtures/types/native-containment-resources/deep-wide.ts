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
interface Boxdeepwide<T> { value: T }
declare function deepdeepwide<T>(): Boxdeepwide<Boxdeepwide<Boxdeepwide<Boxdeepwide<Boxdeepwide<Boxdeepwide<Boxdeepwide<Boxdeepwide<Boxdeepwide<Boxdeepwide<Boxdeepwide<Boxdeepwide<Boxdeepwide<Boxdeepwide<Boxdeepwide<Boxdeepwide<Boxdeepwide<Boxdeepwide<Boxdeepwide<Boxdeepwide<Boxdeepwide<Boxdeepwide<Boxdeepwide<Boxdeepwide<T>>>>>>>>>>>>>>>>>>>>>>>>;
function querydeepwide() {
  const deepChosen = (deepdeepwide<number>(), 0 as number);
  const healthy = 0 as number;
}
