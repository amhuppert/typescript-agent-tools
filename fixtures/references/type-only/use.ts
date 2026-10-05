// Type-only imports and exports alias the same declarations.
import type { /*@Shape*/Shape } from "./types";
import { type /*@Kind*/Kind, /*@kind*/kind } from "./types";
import type { /*@Model*/Model } from "./types";

let /*@shape*/shape: /*@Shape*/Shape = { /*@!*/size: 1 };
let /*@current*/current: /*@Kind*/Kind = /*@kind*/kind;
let /*@model*/model: /*@Model*/Model;
type /*@ModelClass*/ModelClass = typeof /*@Model*/Model;

export type { /*@Shape*/Shape };
export { type /*@Kind*/Kind as /*@Kind*/Alias };
export type { /*@Model*/Model as /*@Model*/Entity } from "./types";
/*@shape*/shape; /*@current*/current; /*@model*/model;
