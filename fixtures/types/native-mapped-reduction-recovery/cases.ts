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

type Mapped<T> = { [K in keyof T]: MissingMappedTemplate };
type Source = { tag: MissingReduction; value: 1 } & { tag: "a"; value: 1 };
declare const mapped: Mapped<Source>;
const /*#homomorphic*/homomorphic = mapped.value;
type Remapped<T> = { [K in keyof T as K]: MissingRemappedTemplate };
type RemapSource = { tag: MissingRemapReduction; value: 1 } & { tag: "a"; value: 1 };
declare const remapped: Remapped<RemapSource>;
const /*#remapped*/remappedResult = remapped.value;
type Optional<T> = { readonly [K in keyof T]?: MissingOptionalTemplate };
type OptionalSource = { tag: MissingOptionalReduction; value: 1 } & { tag: "a"; value: 1 };
declare const optional: Optional<OptionalSource>;
const /*#optional*/optionalResult = optional.value;
type UnionMapped<T> = { [K in keyof T]: MissingUnionTemplate };
type UnionSource = ({ tag: MissingUnionReduction; value: 1 } & { tag: "a"; value: 1 }) | { tag: "b"; value: 2 };
declare const union: UnionMapped<UnionSource>;
const /*#union*/unionResult = union.value;
type NeverMapped<T> = { [K in keyof T]: MissingNeverTemplate };
type NeverSource = { tag: "a"; payload: MissingNeverPayload; value: 1 } & { tag: "b"; value: 1 };
declare const neverMapped: NeverMapped<NeverSource>;
const /*#never*/neverResult = neverMapped.value;
type Copy<T> = { [K in keyof T]: T[K] };
declare const healthyMapped: Copy<{ value: 41 } & { tag: "a" }>;
const /*#healthy*/healthy = healthyMapped.value;
