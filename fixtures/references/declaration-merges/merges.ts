// Valid merges: overloads, an ambient class with a function, interfaces with
// matching type parameters, repeated `infer` names, and a `var` hoisted out
// of a catch block.
export {};
function /*@f:d*/f(): void;
function /*@f:d*/f(/*@x:d*/x?: number) { /*@x:u*/x; }
declare class /*@C:d*/C {}
declare function /*@C:d*/C(): void;
interface /*@Box:d*/Box</*@BoxT:d*/T> { /*@!*/a: /*@BoxT:u*/T }
interface /*@Box:d*/Box</*@BoxT:d*/T> { /*@!*/b: /*@BoxT:u*/T }
type /*@Pair:d*/Pair</*@PairA:d*/A> = /*@PairA:u*/A extends [infer /*@B:d*/B, infer /*@B:d*/B] ? /*@B:u*/B : never;
/*@!*/catch_scope: try {} catch (/*@e:d*/e) { var /*@v:d*/v = /*@e:u*/e; }
/*@f:u*/f(); /*@C:u*/C; let /*@b:d*/b: /*@Box:u*/Box<1> | /*@Pair:u*/Pair<[1, 1]>; /*@b:u*/b; /*@v:u*/v;
