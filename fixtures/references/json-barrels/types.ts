// A JSON value and its properties are values only: a type or namespace use
// finds nothing (TS2694, TS2749, TS2503), while `typeof` reaches them.
import { /*@!*/a } from "./data.json";
import /*@!*/d from "./data.json";
import /*@!*/r = require("./data.json");
import type { /*@!*/a as /*@!*/ta } from "./data.json";
import { "" as /*@!*/empty } from "./data.json";
import { "" as /*@?*/none } from "./other.json";

export type /*@t1:de*/T1 = import("./data.json")./*@?*/a;
export type /*@t2:de*/T2 = typeof import("./data.json")./*@!*/a;
export type /*@t3:de*/T3 = import("./data.json")./*@?*/default;
export type /*@t4:de*/T4 = typeof import("./data.json")./*@!*/default;
export type /*@t5:de*/T5 = typeof import("./other.json")./*@?*/default;
export type /*@t6:de*/T6 = /*@?*/a;
export type /*@t7:de*/T7 = /*@?*/d;
export type /*@t8:de*/T8 = /*@?*/r./*@?*/a;
export type /*@t9:de*/T9 = typeof /*@!*/r./*@!*/a;
export type /*@t10:de*/T10 = typeof /*@!*/r./*@!*/missing;
export type /*@t11:de*/T11 = typeof /*@!*/d./*@!*/nested./*@!*/x;
export type /*@t12:de*/T12 = typeof /*@!*/ta;
/*@?*/ta; /*@!*/empty; /*@?*/none;
