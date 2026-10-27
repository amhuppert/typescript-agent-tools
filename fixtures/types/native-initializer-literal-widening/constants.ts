export const STRING = "string-value";
export const NUMBER = 1;
export const BOOLEAN = true;
export enum Choice { First, Second }
export const MEMBER = Choice.Second;
export const TEMPLATE = `template-value`;
export const SUBSTITUTED = `${STRING}-suffix`;
export const ASSERTED = "asserted-value" as const;
export const RECORD = { key: "record-value" } as const;
export let MUTABLE = "mutable-value";
export var LEGACY = "legacy-value";
export const OBJECT = { key: "object-value" };
export declare const SHAPE: { readonly key: "shape-value" };
