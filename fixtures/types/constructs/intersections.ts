import { Box, Point } from './definitions';

// Intersections whose apparent type TypeScript rebuilds without the alias
// (getTypeWithThisArgument): a class, a generic instantiation, an interface
// with `this`, and one extending a class. The literal's contextual type is
// still the declared type.
export interface Fluent { name: string; tag?: string; set(): this }
export interface FromClass extends Point { label?: string }
export interface Plain { id: string; note?: string }
export type PointExtra = Point & { extra: string; more?: number };
export type BoxExtra = Box<number> & { extra?: string };
export type FluentExtra = Fluent & { extra?: string };
export type ClassExtra = FromClass & { extra?: string };
export type PE<T> = Box<T> & { extra?: string };
export type Props = Box<string> & Plain;
export type Variant = (Point & { k: 'a' }) | { k: 'b'; z?: number };

declare function takePE(value: PE<string>): void;
declare function takeExtra(value: PointExtra): void;

export const pe: PointExtra = { x: 1, extra: 'e', move() {} };
export const be: BoxExtra = { value: 1 };
export const fe: FluentExtra = { name: 'n', set() { return this; } };
export const ce: ClassExtra = { x: 2, move() {}, label: 'l' };
export const pn: PE<number> = { value: 1 };
takePE({ value: 'v' });
export const pes: PE<boolean>[] = [{ value: true }];
export const props: Props = { value: 's', id: 'i' };
export const va: Variant = { k: 'a', x: 1, move() {} };
export const vb: Variant = { k: 'b' };
takeExtra({ x: 3, extra: 'arg', move() {} });
// A type parameter's constraint is what the literal is checked against.
export function make<T extends Plain>(): T { return { id: 'tp' } as T; }
