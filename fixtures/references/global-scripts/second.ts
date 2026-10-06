// `var` and interface declarations merge across global scripts; module-local
// declarations are not globals.
var /*@shared:d*/shared = 2;
interface /*@GlobalShape:d*/GlobalShape { /*@!*/b: number }
/*@globalFn:u*/globalFn();
let /*@shape:d*/shape: /*@GlobalShape:u*/GlobalShape = { /*@!*/a: 1, /*@!*/b: 2 };
/*@shape:u*/shape;
/*@?*/moduleOnly;
