// A namespace creates a value only when its body does, so a namespace that
// exports only a namespace of types shares its name with a value.
declare namespace /*@A:d*/A {
  namespace /*@B:d*/B { type /*@T:d*/T = 1; }
  export { /*@B:e*/B };
}
declare const /*@Avalue:d*/A: number;

// A non-exported `import =` creates no value either.
declare namespace /*@C:d*/C {
  namespace /*@N:d*/N { type /*@U:d*/U = 2; }
  import /*@N:i*/local = /*@N:u*/N;
}
declare const /*@Cvalue:d*/C: number;

// An exported name is looked up outward from the namespace that exports it:
// here an interface, so `E` creates no value beside the constant `E`.
declare namespace /*@D:d*/D {
  interface /*@X:d*/X {}
  namespace /*@E:d*/E { export { /*@X:e*/X }; }
  const /*@Evalue:d*/E: number;
}
