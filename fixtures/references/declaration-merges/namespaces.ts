// A namespace after the function it merges with, an ambient namespace before
// an ambient function, and interfaces whose type parameters match.
export {};
function /*@Nf:d*/f() {}
namespace /*@Nf:d*/f { export const /*@Nx:de*/x = 1; }
declare namespace /*@Ng:d*/g { const /*@Ny:d*/y: number; }
declare function /*@Ng:d*/g(): void;
interface /*@NA:d*/A</*@NAT:d*/T> {}
interface /*@NA:d*/A</*@NAT:d*/T> {}
/*@Nf:u*/f./*@Nx:u*/x; /*@Ng:u*/g./*@Ny:u*/y; let /*@Na:d*/a: /*@NA:u*/A<1>; /*@Na:u*/a;
