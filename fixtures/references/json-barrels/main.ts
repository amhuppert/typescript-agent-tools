import { /*@!*/a, /*@?*/missing, /*@!*/nested } from "./barrel-require";
import { /*@!*/a as /*@!*/a2, /*@?*/missing as /*@?*/missing2 } from "./barrel-default";
import { /*@!*/a as /*@!*/a3, /*@?*/missing as /*@?*/missing3 } from "./barrel-namespace";
import { /*@!*/a as /*@!*/a4, /*@?*/missing as /*@?*/missing4 } from "./outer";
import { /*@!*/default as /*@!*/whole } from "./barrel-require";
import { /*@!*/x, /*@?*/missing as /*@?*/missing5 } from "./nested-barrel";
import { /*@?*/length } from "./string-barrel";
import { /*@!*/a as /*@!*/a5 } from "./star-same";
import { /*@!*/a as /*@!*/a6 } from "./star-alias";
import { /*@!*/v } from "./star-value";
import { /*@?*/a as /*@?*/a7 } from "./star-files";
import { /*@?*/a as /*@?*/a8 } from "./star-declared";
import * as /*@!*/ns from "./barrel-require";

/*@!*/a; /*@?*/missing; /*@!*/nested; /*@!*/a2; /*@?*/missing2; /*@!*/a3; /*@?*/missing3;
/*@!*/a4; /*@?*/missing4; /*@!*/whole; /*@!*/x; /*@?*/missing5; /*@?*/length;
/*@!*/a5; /*@!*/a6; /*@!*/v; /*@?*/a7; /*@?*/a8;
/*@!*/ns./*@!*/a; /*@!*/ns./*@!*/missing;
