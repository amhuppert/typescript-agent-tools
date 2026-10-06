// A CommonJS module takes the `require` condition, whatever its import syntax.
import /*@cjs:i*/cond = require("cond");
import { /*@cjsRun:i*/run, /*@cjsOnly:i*/cjsOnly, /*@?*/esmOnly } from "cond";
/*@cjs:u*/cond./*@cjsRun:u*/run(); /*@cjsRun:u*/run(); /*@cjsOnly:u*/cjsOnly; /*@cjs:u*/cond./*@cjsOnly:u*/cjsOnly;
export {};
