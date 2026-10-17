type UnionLayer4_0 = { readonly leaf: number };
type UnionLayer4_1 = Promise<UnionLayer4_0> | { readonly alternative1: number };
type UnionLayer4_2 = Promise<UnionLayer4_1> | { readonly alternative2: number };
type UnionLayer4_3 = Promise<UnionLayer4_2> | { readonly alternative3: number };
type UnionLayer4_4 = Promise<UnionLayer4_3> | { readonly alternative4: number };
declare const unionPending4_1: UnionLayer4_1;
declare const unionPending4_2: UnionLayer4_2;
declare const unionPending4_3: UnionLayer4_3;
declare const unionPending4_4: UnionLayer4_4;
async function queryUnionStack4() {
  const chosen = ({
    warm1: unionPending4_1,
    warm2: unionPending4_2,
    warm3: unionPending4_3,
    warm4: unionPending4_4,
    result: await unionPending4_4,
  }, 0 as number);
  const healthy = 0 as number;
}
