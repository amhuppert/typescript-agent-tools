export interface View { id: string; status: 'open' | 'closed'; lane?: string; note?: string }
export interface Box<T> { value: T; label?: string }
export type Pair = { left: number; right?: number };
export type Tagged<T> = { tag: T; extra?: string };
export type Merged = View & { owner: string; due?: number };
export class Point { x = 0; y?: number; move(): void {} }
export interface WithMethod { run(): void; title?: string; get size(): number }
export interface Base { serial: number; origin?: string }
export interface Derived extends Base { kind: 'derived' }
export type Alias = View;
export type Fixed = Box<number>;
export type PartialView = Partial<View>;
export type Cross<T> = ({ id: T } & { a?: number }) | ({ other: T } & { b?: number });
export interface Options { lane?: string; depth?: number }
export class Hidden { private p?: number; visible = 1 }
export type Keyed = { p: number; id: number };
export type Bag = { [key: string]: number; size?: number };
export type Numbered = { 1: string; two?: string; '-1'?: string };
export interface Empty {}
