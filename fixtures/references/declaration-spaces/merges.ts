// A namespace of const enums only is instantiated for merge order only when
// const enums are preserved, so here it may precede its function.
export {};
namespace /*@f:d*/f { export const enum /*@E:de*/E { /*@!*/A } }
function /*@f:d*/f() {}
/*@f:u*/f();
let /*@e:d*/e = /*@f:u*/f./*@E:u*/E./*@!*/A;
/*@e:u*/e;

// An exported interface and a local namespace of the same name occupy
// different declaration spaces, so neither TS2395 nor a shared group.
export interface /*@Shape:de*/Shape { /*@!*/kind: string }
namespace /*@ShapeNs:d*/Shape { export type /*@Kind:de*/Kind = string; }
let /*@s:d*/s: /*@Shape:u*/Shape = { /*@!*/kind: "" };
let /*@k:d*/k: /*@ShapeNs:u*/Shape./*@Kind:u*/Kind = "";
/*@s:u*/s; /*@k:u*/k;
