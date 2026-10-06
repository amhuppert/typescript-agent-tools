// `globalThis` qualifies global `var`, function, and type declarations, but
// not block-scoped ones. `undefined` is a built-in value, not a target.
/*@!*/globalThis./*@shared:u*/shared; /*@!*/globalThis./*@globalFn:u*/globalFn();
/*@!*/globalThis./*@?*/blockScoped;
let /*@qualified:d*/qualified: /*@!*/globalThis./*@GlobalShape:u*/GlobalShape;
/*@qualified:u*/qualified; /*@!*/undefined;
export {};
