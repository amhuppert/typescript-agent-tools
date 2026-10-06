// A named re-export cycle through a renaming: every alias in it is
// unresolved (TypeScript's TS2303).
export { /*@?*/looped } from "./loop-b";
