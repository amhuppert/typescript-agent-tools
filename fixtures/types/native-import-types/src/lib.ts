export interface Box<T> { /*#boxValue*/value: T }
export class Thing { readonly kind = "thing" }
export const value = { /*#count*/count: 1, /*#nested*/nested: { /*#deep*/deep: "x" } };
export function make<T>(input: T): Box<T> { return { value: input }; }
export namespace Outer {
    export namespace Inner {
        export interface Leaf { leaf: true }
        export const marker = 1;
    }
}
export { Box as Crate };
export type { Thing as ThingType };
export type Name = "a" | "b";
export interface Limited<T extends string> { limited: T }
export type { value as typeOnlyValue };
export type Pair<T> = { first: T; second: T };
export type Wide<T extends string = "d"> = { wide: T };
