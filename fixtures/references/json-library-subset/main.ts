// `es2015.core` alone declares only part of `Array` and `String`: a member
// that only `es5` declares is missing.
import { /*@?*/length, /*@!*/find } from "./array.json";
import { /*@?*/charAt, /*@!*/startsWith } from "./string.json";
/*@?*/length; /*@!*/find; /*@?*/charAt; /*@!*/startsWith;
