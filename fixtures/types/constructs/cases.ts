import { Alias, Base, Box, Derived, Merged, Pair, Point, Tagged, View, WithMethod } from './definitions';

declare function takeView(view: View): void;
declare function takeViews(views: View[]): void;
declare function generic<T>(value: T): T;
declare function maybeView(view?: View): void;
declare function overloaded(value: number): void;
declare function overloaded(value: View): void;
declare const partialView: Partial<View>;
declare const fullView: View;
declare const anyValue: any;
declare const cond: boolean;

export const annotated: View = { id: 'a', status: 'open' };
export const satisfied = { id: 'b', status: 'open', lane: 'x' } satisfies View;
export const asserted = { id: 'c' } as View;
export function returns(): View { return { id: 'd', status: 'closed', lane: 'l', note: 'n' }; }
export async function asyncReturns(): Promise<View> { return { id: 'e', status: 'open' }; }
takeView({ id: 'f', status: 'open' });
takeViews([{ id: 'g', status: 'open' }, { id: 'h', status: 'closed', note: '' }]);
maybeView({ id: 'i', status: 'open' });
generic<View>({ id: 'j', status: 'open' });
export const inferred = generic({ id: 'k', status: 'open' });
overloaded({ id: 'l', status: 'open' });
export const nested: { view: View } = { view: { id: 'm', status: 'open' } };
export function defaulted(view: View = { id: 'n', status: 'open' }) { return view; }
let assigned: View;
assigned = { id: 'o', status: 'open' };
export const spreadFull: View = { ...fullView, id: 'p' };
export const spreadPartial: View = { ...partialView, id: 'q', status: 'open' };
export const spreadAny: View = { ...anyValue, id: 'r' };
export const conditional: View = { id: 's', status: 'open', ...(cond ? { lane: 'x' } : {}) };
export const andSpread: View = { id: 't', status: 'open', ...(cond && { note: 'y' }) };
export const computed: View = { ['id']: 'u', status: 'open' };
const KEY = 'lane' as const;
export const lateBound: View = { id: 'v', status: 'open', [KEY]: 'w' };
export const arrow = (id: string): View => ({ id, status: 'open' });
export const unrelated = { id: 'w', other: 1 };
export const viaAlias: Alias = { id: 'x', status: 'open' };
export const partial: Partial<View> = { id: 'y' };

export const box: Box<string> = { value: 'v' };
export const boxes: Box<number>[] = [{ value: 1, label: 'one' }];
export const pair: Pair = { left: 1 };
export const tagged: Tagged<'t'> = { tag: 't' };
export const maybeTagged: Tagged<number> | undefined = { tag: 1, extra: 'e' };
export const merged: Merged = { id: 'z', status: 'open', owner: 'me' };
export const point: Point = { x: 1, move() {} };
export const method: WithMethod = { run() {}, get size() { return 1; } };
export const derived: Derived = { serial: 1, kind: 'derived' };
export const base: Base = { serial: 2 };

declare function takeTwo(view: View, count: number): void;
export class Holder { constructor(public view: View) {} }
takeView({ id: 'missing-status' });
takeTwo({ id: 'sibling', status: 'open' }, 'not a number');
export const held = new Holder({ id: 'held', status: 'closed' });
export const broken = new Holder({ id: 'broken' });
export const iife = ((view: View) => view)({ id: 'iife', status: 'open' });
export const inferredIife = ((value) => value)({ id: 'inferred-iife', status: 'open' });
