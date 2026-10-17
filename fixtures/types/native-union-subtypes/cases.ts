interface Base { x: number }
interface Derived { x: number; y: string }
interface Alpha { kind: "a"; x: number }
interface Beta { kind: "b"; x: number }
declare const base: Base, derived: Derived, alpha: Alpha, beta: Beta;
function one(flag: boolean) { if (flag) return base; return derived; }
const /*#base*/baseResult = one(true);
function reversed(flag: boolean) { if (flag) return derived; return base; }
const /*#reverse*/reverseResult = reversed(true);
function discriminated(flag: boolean) { if (flag) return alpha; return beta; }
const /*#discriminated*/discriminatedResult = discriminated(true);
function constrained<T extends Alpha | Beta>(flag: boolean, value: T) {
  function collect() {
    if (flag) return value;
    if (flag) return alpha;
    return beta;
  }
  const /*#constrained*/result = collect();
}
class Parent { x: number }
class Child extends Parent { y: string }
class Sibling { x: number }
declare const parent: Parent, child: Child, sibling: Sibling;
function hierarchy(flag: boolean) { if (flag) return parent; return child; }
const /*#hierarchy*/hierarchyResult = hierarchy(true);
function unrelated(flag: boolean) { if (flag) return parent; return sibling; }
const /*#unrelated*/unrelatedResult = unrelated(true);
interface Empty {}
declare const empty: Empty;
function emptiness(flag: boolean) { if (flag) return empty; return 1; }
const /*#empty*/emptyResult = emptiness(true);
function primitive(flag: boolean) { if (flag) return 1; return "x"; }
const /*#primitive*/primitiveResult = primitive(true);
type Z = [string];
type A = [number];
export declare const /*#aliases*/tupleAliases: Z | A;
