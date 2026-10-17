declare module "scope-merge-target" { export const value: 81; }
declare module "scope-merge" {
    export { value as MixedEntry, value as PureEntry } from "scope-merge-target";
}
