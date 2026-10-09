// Each file's declarations of an ambient module merge into the module of
// that name in program order, after the global table (`mergeGlobalSymbol`).
// The earlier file's alias meets the later file's interface: the interface
// merges with what the alias names.
declare namespace /*@Other:d*/Other {
  interface /*@X:d*/X { /*@!*/a: number }
}
declare module "aliased" {
  export import /*@X:ei*/X = /*@Other:u*/Other./*@X:u*/X;
}
// The first declaration decides whether the merged module is shorthand.
declare module "short";
// Pattern ambient modules declared in several files are one module.
declare module "*.view" {
  export interface /*@A:de*/A {}
}
