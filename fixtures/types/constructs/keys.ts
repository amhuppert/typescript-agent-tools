// Key kinds and spellings: a private name, a unique symbol, and string keys
// that are not identifiers.
declare const sym: unique symbol;
export class Secret { #hidden = 1; shown?: number }
export interface Symbolic { [sym]: number; label?: string }
export interface TextKeys { 'lane?': number; 'status (unknown)': number; 'x\ny': number; '': number }

export const secret: Secret = { shown: 1 };
export const symbolic: Symbolic = { label: 'x' };
export const symbolicWritten: Symbolic = { [sym]: 1 };
export const text: TextKeys = {};
