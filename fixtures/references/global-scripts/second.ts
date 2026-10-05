// `var` and interface declarations merge across global scripts; module-local
// declarations are not globals.
var /*@shared*/shared = 2;
interface /*@GlobalShape*/GlobalShape { /*@!*/b: number }
/*@globalFn*/globalFn();
let /*@shape*/shape: /*@GlobalShape*/GlobalShape = { /*@!*/a: 1, /*@!*/b: 2 };
/*@shape*/shape;
/*@?*/moduleOnly;
