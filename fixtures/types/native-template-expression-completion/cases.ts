function substitutions<A extends MissingSubstitutionFirst, B extends MissingSubstitutionSecond>(first: A, second: B) {
  const /*#substitutions*/result = `x${first}y${second}z`;
}
function constantContext<A extends MissingConstFirst, B extends MissingConstSecond>(first: A, second: B) {
  const /*#constantContext*/result = `x${first}y${second}` as const;
}
function elementContext<A extends MissingElementFirst, B extends MissingElementSecond>(first: A, second: B, table: { [key: string]: 31 }) {
  const /*#elementContext*/result = table[`x${first}y${second}`];
}
function contextual<C extends MissingContext, L extends MissingContextLater>(part: number, later: L, sink: <Unused>(text: C, last: L) => 21) {
  const /*#contextual*/result = sink(`x${part}`, later);
}
function healthyRuntime(part: number) {
  const /*#healthyRuntime*/result = `x${part}y`;
}
const /*#healthyConstant*/constant = `x${1}y`;
function healthyContext(part: number, sink: <Unused>(text: `x${number}`) => 41) {
  const /*#healthyContext*/result = sink(`x${part}`);
}
function symbolConversion(value: symbol) {
  const /*#symbolConversion*/result = `x${value}y`;
}
function contextualConditional<C, L extends MissingConditionalLater>(part: number, later: L,
    sink: <Unused>(text: C extends string ? MissingConditionalTrue : MissingConditionalFalse, last: L) => 51) {
  const /*#contextualConditional*/result = sink(`x${part}`, later);
}
