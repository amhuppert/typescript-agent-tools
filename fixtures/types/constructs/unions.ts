import { View } from './definitions';

export type Shape = { kind: 'circle'; radius: number; color?: string } | { kind: 'square'; side: number; color?: string };
export type Result<T> = { ok: true; data: T; warning?: string } | { ok: false; error: string };
export interface First { type: 'a'; a: number; shared?: string }
export interface Second { type: 'b'; b?: number; shared?: string }
export type Event = First | Second;
export interface Other { id: number; other: boolean }

export const circle: Shape = { kind: 'circle', radius: 1 };
export const square: Shape = { kind: 'square', side: 2, color: 'red' };
export const shapes: Shape[] = [{ kind: 'circle', radius: 3 }];
export const ok: Result<string> = { ok: true, data: 'd' };
export const failed: Result<string> = { ok: false, error: 'e' };
export const optionalResult: { result?: Result<number> } = { result: { ok: true, data: 1 } };
export const first: Event = { type: 'a', a: 1 };
export const second: Event = { type: 'b' };
export const undiscriminated: Event = { shared: 's', a: 1 } as Event;
export const either: View | Other = { id: 'id', status: 'open' };
export const ambiguous: View | Other = { id: 1, other: true };
