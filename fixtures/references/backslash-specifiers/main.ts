// A relative specifier may use `\` separators, as TypeScript's
// `pathIsRelative` and `combinePaths` read them.
import { /*@value:i*/value } from ".\\sub\\a";
import { /*@value:i*/value as /*@value:i*/other } from "./sub\\a";
/*@value:u*/value; /*@value:u*/other;
