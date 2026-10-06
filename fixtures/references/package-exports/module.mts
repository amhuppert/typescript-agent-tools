// An ES module takes the `import` condition, and `customConditions` puts
// `custom` before `default`; `typesVersions` does not apply beside `exports`.
// Without `exports`, the first matching `typesVersions` range remaps the
// package root and its subpaths.
import { /*@esmRun:i*/run, /*@esmOnly:i*/esmOnly, /*@?*/cjsOnly, /*@?*/ignored } from "cond";
import { /*@feature:i*/feature, /*@?*/fallback } from "cond/feature";
import { /*@legacy:i*/legacy, /*@?*/rootOnly, /*@?*/oldOnly } from "legacy";
import { /*@sub:i*/sub } from "legacy/sub.js";
/*@esmRun:u*/run(); /*@esmOnly:u*/esmOnly; /*@feature:u*/feature; /*@legacy:u*/legacy; /*@sub:u*/sub;
