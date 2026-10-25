// A literal's own lookup falls back to the global Object type's named
// members only: Object's call and construct signatures reach no Function
// member, and its index signature makes no other key present.
export interface Callish { call?: unknown; apply?: unknown; bind?: unknown; marker?: string }
export interface Indexed { marker?: string; custom: string; maybe?: number; own?: boolean }

export const plain: Callish = {};
export const spread: Callish = { ...{} };
export const marked: Callish = { marker: 'x' };
export const empty: Indexed = {};
export const owned: Indexed = { own: true };
export const spreadIndexed: Indexed = { ...{} };
