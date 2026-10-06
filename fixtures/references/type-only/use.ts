// Type-only imports and exports alias the same declarations.
import type { /*@Shape:i*/Shape } from "./types";
import { type /*@Kind:i*/Kind, /*@kind:i*/kind } from "./types";
import type { /*@Model:i*/Model } from "./types";

let /*@shape:d*/shape: /*@Shape:u*/Shape = { /*@!*/size: 1 };
let /*@current:d*/current: /*@Kind:u*/Kind = /*@kind:u*/kind;
let /*@model:d*/model: /*@Model:u*/Model;
type /*@ModelClass:d*/ModelClass = typeof /*@Model:u*/Model;

export type { /*@Shape:e*/Shape };
export { type /*@Kind:e*/Kind as /*@Kind:e*/Alias };
export type { /*@Model:e*/Model as /*@Model:e*/Entity } from "./types";
/*@shape:u*/shape; /*@current:u*/current; /*@model:u*/model;
