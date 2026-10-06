// Same-named declarations in different meanings stay separate symbols unless
// they validly merge. Lookup follows the meaning the position requires.
type /*@typeThing:d*/Thing = { /*@!*/kind: string };
const /*@valueThing:d*/Thing = { /*@!*/kind: "value" };
let /*@x:d*/x: /*@typeThing:u*/Thing = /*@valueThing:u*/Thing;

interface /*@typeBox:d*/Box { /*@!*/size: number }
declare var /*@valueBox:d*/Box: { new (): /*@typeBox:u*/Box };
let /*@b:d*/b: /*@typeBox:u*/Box = new /*@valueBox:u*/Box();

class /*@Pair:d*/Pair {}
let /*@p:d*/p: /*@Pair:u*/Pair = new /*@Pair:u*/Pair();

namespace /*@space:d*/Space { export type /*@Inner:de*/Inner = number; }
const /*@spaceValue:d*/Space = 1;
let /*@z:d*/z: /*@space:u*/Space./*@Inner:u*/Inner = /*@spaceValue:u*/Space;

type /*@Query:d*/Query = typeof /*@valueThing:u*/Thing;
let /*@k:d*/k: typeof /*@x:u*/x;
let /*@q:d*/q: typeof /*@valueThing:u*/Thing./*@!*/kind;

function /*@scope:d*/scope() {
  const /*@innerValue:d*/Thing = 1;
  let /*@y:d*/y: /*@typeThing:u*/Thing = { /*@!*/kind: "" };
  return /*@innerValue:u*/Thing + /*@y:u*/y./*@!*/kind;
}

export { /*@typeThing,valueThing:e*/Thing, /*@typeBox,valueBox:e*/Box, /*@space,spaceValue:e*/Space };
export type { /*@Query:e*/Query, /*@Pair:e*/Pair };
/*@b:u*/b; /*@p:u*/p; /*@z:u*/z; /*@k:u*/k; /*@q:u*/q; /*@scope:u*/scope();
