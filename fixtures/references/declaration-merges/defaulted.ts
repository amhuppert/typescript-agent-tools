// A merged declaration may add type parameters that have defaults.
export {};
interface /*@DA:d*/A</*@DAT:d*/T> {}
interface /*@DA:d*/A</*@DAT:d*/T, /*@DAU:d*/U = number> {}
class /*@DD:d*/D</*@DDT:d*/T> {}
interface /*@DD:d*/D</*@DDT:d*/T, /*@DDU:d*/U = string> {}
interface /*@DI:d*/I</*@DIT:d*/T = 1> {}
interface /*@DI:d*/I {}
let /*@Da:d*/a: /*@DA:u*/A<1>; let /*@Dd:d*/d: /*@DD:u*/D<1>; let /*@Di:d*/i: /*@DI:u*/I;
/*@Da:u*/a; /*@Dd:u*/d; /*@Di:u*/i;
