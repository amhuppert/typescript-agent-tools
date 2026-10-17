interface Object {}
interface Function {}
interface CallableFunction {}
interface NewableFunction {}
interface String {}
interface Number {}
interface Boolean {}
interface BigInt {}
interface Symbol {}
interface Array<T> { readonly length: number; [n: number]: T; }
interface ReadonlyArray<T> { readonly length: number; readonly [n: number]: T; }
interface Array<T> {
  filter<S extends T>(test: (value: T) => value is S): S[];
  filter(test: (value: T) => unknown): T[];
}
interface ReadonlyArray<T> {
  filter<S extends T>(test: (value: T) => value is S): readonly S[];
  filter(test: (value: T) => unknown): readonly T[];
}
