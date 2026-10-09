// A JSON module's only export is `export=`, its value: default, namespace,
// and `default` imports name it, and named imports name its top-level
// properties. Neither is a reference target; an absent name is unresolved.
import /*@!*/data from "./data.json";
import { /*@!*/greeting, /*@?*/absent } from "./data.json";
import * as /*@!*/ns from "./data.json";
import { /*@!*/default as /*@!*/value } from "./data.json";
export { /*@!*/greeting as /*@!*/salutation, /*@!*/count } from "./data.json";
export * as /*@!*/everything from "./data.json";

export function /*@show:de*/show(/*@text:d*/text: string): string {
  return /*@text:u*/text;
}

/*@show:u*/show(/*@!*/data./*@!*/greeting);
/*@show:u*/show(/*@!*/greeting);
/*@!*/ns./*@!*/nested./*@!*/inner;
/*@!*/value;
/*@?*/absent;
