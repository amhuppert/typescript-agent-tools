import { Fixed, Point } from './definitions';
import { Fluent, Plain, PointExtra } from './intersections';

// A type parameter's base constraint is the declared type itself, though its
// apparent form is rebuilt with a `this` argument (a class, an alias of an
// instantiation, an intersection, a `this`-typed interface).
export function toPoint<T extends Point>(): T { return { x: 1, move() {} } as T; }
export function toFixed<T extends Fixed>(): T { return { value: 1 } as T; }
export function toExtra<T extends PointExtra>(): T { return { x: 1, extra: 'e', move() {} } as T; }
export function toFluent<T extends Fluent>(): T { return { name: 'n', set() { return this; } } as T; }
export function toEither<T extends Plain | Point>(): T { return { id: 'u' } as T; }
// A class's `this` type is constrained by the class.
export class Chain {
  value = 0; note?: string;
  clone(): this { return { value: 0, clone: this.clone } as this; }
}
// One alternative reached twice is one construction.
export function twice<T extends Plain>(): T | Plain { return { id: 'twice' }; }
// A computed name takes its literal type's key: `[+100]` writes `100`, `[-0]` writes `0`.
export interface Signed { '100'?: string; '0'?: string; tag?: string }
export const plus: Signed = { [+100]: 'v', tag: 'a' };
export const minusZero: Signed = { [-0]: 'v', tag: 'b' };
export const plusOnly: Signed = { [+100]: 'v' };
// An object literal has no call signature, so a function type is no alternative.
export type Fn = () => void;
export type Callable = Fn | Plain;
export const callable: Callable = { id: 'c' };
// An accessor keeps the binder's name: only property assignments, shorthand
// and methods take a computed name's type key (`get [+100]` names `+100`).
export interface Accessed { '100'?: string; '+100'?: string; '0'?: string; '-0'?: string; tag?: string }
export interface GetterOnly { '+100'?: string }
export const getter: Accessed = { get [+100]() { return 'g'; }, tag: 'g' };
export const setter: Accessed = { set [-0](value: string) {}, tag: 's' };
export const data: Accessed = { [+100]: 'd', tag: 'd' };
export const getterOnly: GetterOnly = { get [+100]() { return 'g'; } };
