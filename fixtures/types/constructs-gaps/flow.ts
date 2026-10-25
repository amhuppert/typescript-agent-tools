import { View } from './definitions';

declare function pick<T>(a: T, b: T): T;

export function narrowedArgument(value: View | undefined) {
  if (value) pick(value, { id: 'a', status: 'open' });
}
export function narrowedMember(id: string | undefined): View | undefined {
  if (!id) return undefined;
  return { id, status: 'open' };
}
export function narrowedSpread(base: View | undefined): View | undefined {
  if (!base) return undefined;
  return { ...base, lane: 'b' };
}
export class Holder {
  view?: View;
  reset() { this.view = { id: 'c', status: 'closed' }; }
}
declare function respond(body: unknown, init?: { status: number }): void;
export function report(error: string | undefined) {
  if (error) respond({ id: error, status: 'open' } satisfies View, { status: 500 });
}
declare function either(value: number): void;
declare function either(value: View): void;
either({ id: 'no-applicable-overload' });
type Circle = { kind: 'circle'; id: string };
type Square = { kind: 'square'; id: string };
// No member of its context is a target alternative: not a site, and no gap
// though discriminating it would read a narrowed value.
export function otherUnion(kind: 'circle' | 'square' | undefined) {
  if (kind) { const shape: Circle | Square = { kind, id: 'x' }; return shape; }
}
