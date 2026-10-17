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

declare const /*#directAny*/directAny: any extends string ? MissingDirectTrue : MissingDirectFalse;
type Choose<T> = T extends string ? MissingGenericTrue : MissingGenericFalse;
declare const /*#genericAny*/genericAny: Choose<any>;
declare const /*#distributed*/distributed: Choose<"text" | 0>;
declare const /*#trueOnly*/trueOnly: "text" extends string ? 41 : MissingUnusedFalse;
declare const /*#falseOnly*/falseOnly: 0 extends string ? MissingUnusedTrue : 42;
declare const /*#errorAlias*/errorAlias: MissingCheck extends string ? MissingCheckTrue : MissingCheckFalse;
declare const /*#bareCheck*/bareCheck: number["missing"] extends MissingExtends ? MissingUnreachedTrue : MissingUnreachedFalse;
declare const /*#bareExtends*/bareExtends: MissingBefore extends number["missing"] ? MissingUnreachedThen : MissingUnreachedElse;
declare const /*#inference*/inference: { value: MissingInferSource } extends { value: infer U } ? [U, MissingInferAfter] : never;
type TailNext<T> = T extends number ? MissingTailTrue : MissingTailFalse;
type Tail<T> = T extends string ? 43 : TailNext<T>;
declare const /*#tailFalse*/tailFalse: Tail<0>;
type Nested<T> = T extends "a" ? MissingNestedFirst : T extends "b" ? MissingNestedSecond : MissingNestedLast;
declare const /*#nestedAny*/nestedAny: Nested<any>;
type Key<K> = K extends "x" ? MissingMappedKey : "other";
type ConditionalMap<T extends { x: 0 }> = { [P in Key<keyof T>]: MissingMappedValue };
function mapped<T extends { x: 0 }>(value: ConditionalMap<T>) {
    const /*#mappedLowerBound*/selected = value.x;
}
type HealthyKey<K> = K extends "x" ? "x" : "other";
type HealthyMap<T extends { x: 0 }> = { [P in HealthyKey<keyof T>]: 44 };
function healthyMapped<T extends { x: 0 }>(value: HealthyMap<T>) {
    const /*#healthyLowerBound*/selected = value.x;
}
declare const /*#repeatedInstantiation*/repeatedInstantiation: [Choose<any>, Choose<any>];
const /*#healthy*/healthy = 45 as const;
