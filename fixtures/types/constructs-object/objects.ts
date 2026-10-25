// Property lookup falls back to the global Object type, which a module can
// augment: those members are present, or unknown when optional, though no
// literal writes them.
export interface V { toString(): string; valueOf(): Object; hasOwnProperty(v: string): boolean; marker?: string }
export interface Augmented { custom: string; maybe?: number; own?: boolean }
declare global { interface Object { custom: string; maybe?: number } }

export const empty: V = {};
export const named: V = { marker: 'x' };
export const spread: V = { ...{} };
export const computed: V = { ['marker']: 'x' };
export const plain: Augmented = {};
export const written: Augmented = { own: true };
export const spreadAugmented: Augmented = { ...{} };
