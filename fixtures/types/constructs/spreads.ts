import { Box, Fixed, Hidden, Keyed, Options, PartialView, View } from './definitions';

declare const defaults: Options;
declare const empty: {};
declare const hidden: Hidden;
declare const record: { [key: string]: string };
declare const unknownValue: unknown;
declare const dynamicKey: string;
declare const fallback: Options | undefined;
declare function maybe(): Promise<{ depth: number }>;

export const blank: Options = {};
export const copied: Options = { ...defaults };
export const fromEmpty: Options = { ...empty };
export const dynamic: Options = { [dynamicKey]: 'main' };
export const indexed: Options = { ...record };
export const suppressed: Keyed = { id: 1, p: 1, ...hidden };
export const restored: Keyed = { ...hidden, id: 1, p: 1 };
export function reduced<T extends { p?: 'a' }>(t: T): Keyed { return { id: 1, p: 1, ...t } as Keyed; }
export const invalid: Options = { ...unknownValue } as Options;
export async function awaited() { const value: Options = { lane: 'l', ...await { depth: 2 } }; return value; }
export const either: Options = { lane: 'l', ...(fallback || { depth: 3 }) };
let lane: string | undefined;
({ lane } = { lane: 'x' });
export const fixed: Fixed = { value: 1 };
export const box: Box<number> = { value: 2 };
export const boxString: Box<string> = { value: 's' };
export const partialView: PartialView = { id: 'p' };
export const plainPartial: Partial<View> = { id: 'q' };
export const nestedEmpty: { options: Options } = { options: {} };
