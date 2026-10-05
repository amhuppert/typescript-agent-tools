// Same-named declarations in different meanings stay separate symbols unless
// they validly merge. Lookup follows the meaning the position requires.
type /*@typeThing*/Thing = { /*@!*/kind: string };
const /*@valueThing*/Thing = { /*@!*/kind: "value" };
let /*@x*/x: /*@typeThing*/Thing = /*@valueThing*/Thing;

interface /*@typeBox*/Box { /*@!*/size: number }
declare var /*@valueBox*/Box: { new (): /*@typeBox*/Box };
let /*@b*/b: /*@typeBox*/Box = new /*@valueBox*/Box();

class /*@Pair*/Pair {}
let /*@p*/p: /*@Pair*/Pair = new /*@Pair*/Pair();

namespace /*@space*/Space { export type /*@Inner*/Inner = number; }
const /*@spaceValue*/Space = 1;
let /*@z*/z: /*@space*/Space./*@Inner*/Inner = /*@spaceValue*/Space;

type /*@Query*/Query = typeof /*@valueThing*/Thing;
let /*@k*/k: typeof /*@x*/x;
let /*@q*/q: typeof /*@valueThing*/Thing./*@!*/kind;

function /*@scope*/scope() {
  const /*@innerValue*/Thing = 1;
  let /*@y*/y: /*@typeThing*/Thing = { /*@!*/kind: "" };
  return /*@innerValue*/Thing + /*@y*/y./*@!*/kind;
}

export { /*@typeThing,valueThing*/Thing, /*@typeBox,valueBox*/Box, /*@space,spaceValue*/Space };
export type { /*@Query*/Query, /*@Pair*/Pair };
/*@b*/b; /*@p*/p; /*@z*/z; /*@k*/k; /*@q*/q; /*@scope*/scope();
