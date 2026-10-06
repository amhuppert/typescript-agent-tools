// Decorators in positions TypeScript's checker rejects, and an `infer`
// outside a conditional type, still resolve and declare their names.
declare function /*@dec:d*/dec</*@T:d*/T>(/*@target:d*/target: /*@T:u*/T): /*@T:u*/T;
namespace /*@M:d*/M { export var /*@X:de*/X: number; }
namespace /*@N:d*/N { @/*@dec:u*/dec import /*@X:i*/Y = /*@M:u*/M./*@X:u*/X; }
class /*@C:d*/C { @/*@dec:u*/dec static {} }
type /*@Orphan:d*/Orphan = (infer /*@U:d*/U)[];
