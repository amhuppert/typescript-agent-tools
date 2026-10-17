export {};
type Pair = string | number;
type Other = { other: 0 } | { other: 1 };
type NullablePair = string | null;
type SharedA = string | number;
type SharedB = string | boolean;
type Product = ({ a: 0 } | { a: 1 }) & ({ b: 0 } | { b: 1 });
declare let bottom: never;
declare let onlyUndefined: undefined;
declare let scalar: string;
declare let plain: string | number;
declare let plainNullable: string | number | undefined;
declare let namedNullable: Pair | undefined;
declare let rebuilt: Pair | undefined | boolean;
declare let twoNamed: Pair | Other | undefined;
declare let innerNullable: NullablePair | number;
declare let innerAndOuterNullable: NullablePair | number | undefined;
declare let overlaps: SharedA | SharedB | undefined;
declare let productNullable: Product | undefined;
declare let product: Product;
const /*#never*/neverResult = bottom!;
const /*#scalar*/scalarResult = scalar!;
const /*#scalarRemoved*/scalarRemoved = onlyUndefined!;
const /*#plainUnchanged*/plainUnchanged = plain!;
const /*#plainRemoved*/plainRemoved = plainNullable!;
const /*#singletonOrigin*/singletonOrigin = namedNullable!;
const /*#rebuiltOrigin*/rebuiltOrigin = rebuilt!;
const /*#twoOrigins*/twoOrigins = twoNamed!;
const /*#discardInnerOrigin*/discardInnerOrigin = innerNullable!;
const /*#discardInnerAndOuter*/discardInnerAndOuter = innerAndOuterNullable!;
const /*#overlappingAliases*/overlappingAliases = overlaps!;
const /*#intersectionChanged*/intersectionChanged = productNullable!;
const /*#intersectionUnchanged*/intersectionUnchanged = product!;
function equality(value: Pair | undefined) {
  if (value === undefined) return;
  const equalityRead = /*#equalityFilter*/value;
}
type Tagged = { kind: "a"; payload: 0 } | { kind: "b"; payload: 1 };
function discriminant(value: Tagged | { kind: "c"; payload: 2 }) {
  if (value.kind === "c") return;
  const discriminantRead = /*#discriminantFilter*/value;
}
function assignment(value: Pair | undefined) {
  value = scalar;
  const assignmentRead = /*#assignmentFilter*/value;
}
function defaultSwitch(value: Pair | undefined) {
  switch (typeof value) {
    case "undefined": return;
    default: { const switchRead = /*#switchFilter*/value; }
  }
}

function intersectionFilter(value: Product) {
  if (value.a === 0) { const intersectionRead = /*#intersectionOriginFilter*/value; }
}
