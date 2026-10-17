function plus<A extends MissingPlusFirst, B extends MissingPlusSecond>() {
  const /*#unaryPlus*/result = (+(null as A), +(null as B));
}
function minus<A extends MissingMinusFirst, B extends MissingMinusSecond>() {
  const /*#unaryMinus*/result = (-(null as A), -(null as B));
}
function tilde<A extends MissingTildeFirst, B extends MissingTildeSecond>() {
  const /*#unaryTilde*/result = (~(null as A), ~(null as B));
}
function logicalNot<A extends MissingNotFirst, B extends MissingNotSecond>() {
  const /*#unaryNot*/result = (!(null as A), !(null as B));
}
function prefixUpdate<A extends MissingPrefixFirst, B extends MissingPrefixSecond>() {
  const /*#prefixUpdate*/result = (++(null as A), ++(null as B));
}
function postfixUpdate<A extends MissingPostfixFirst, B extends MissingPostfixSecond>() {
  const /*#postfixUpdate*/result = ((null as A)++, (null as B)++);
}
function arithmetic<A extends MissingArithmeticFirst, B extends MissingArithmeticSecond>() {
  const /*#arithmetic*/result = (null as A) - (null as B);
}
function addition<A extends MissingAdditionFirst, B extends MissingAdditionSecond>() {
  const /*#addition*/result = (null as A) + (null as B);
}
function relational<A extends MissingRelationalFirst, B extends MissingRelationalSecond>() {
  const /*#relational*/result = (null as A) < (null as B);
}
function equality<A extends MissingEqualityFirst, B extends MissingEqualitySecond>(sink: <Unused>(value: boolean) => 61) {
  const /*#equality*/result = sink((null as A) === (null as B));
}
function equalityInitializer<A extends MissingInitializerFirst, B extends MissingInitializerSecond>() {
  const /*#equalityInitializer*/result = (null as A) === (null as B);
}
function compound<A extends MissingCompoundFirst>(host: { get n(): A; set n(value: MissingCompoundLater) }) {
  const /*#compound*/result = host.n -= 1;
}
const /*#numberLiteral*/numberLiteral = -1;
const /*#bigintLiteral*/bigintLiteral = -1n;
const /*#booleanBitwise*/booleanBitwise = true | false;
function symbolShortCircuit<S extends MissingSymbolSkipped>() {
  const /*#symbolShortCircuit*/result = (null as symbol) < (null as S);
}
