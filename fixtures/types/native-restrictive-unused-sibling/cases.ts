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

type Filter<T> = T extends { missing: MissingMarker; value: 41 } ? T : never;
function f<T>(value: Filter<T>) { const /*#query*/query = value.value; }
const /*#healthy*/healthy = 45 as const;
