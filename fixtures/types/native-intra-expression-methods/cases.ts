interface Object {}
interface Function {}
interface Array<T> { length: number; [n: number]: T }
interface ReadonlyArray<T> { readonly length: number; readonly [n: number]: T }
interface String { readonly length: number }
interface Number {}
interface Boolean {}
interface RegExp {}
interface IArguments {}
interface CallableFunction extends Function {}
interface NewableFunction extends Function {}

declare function use<T>(input: { produce: (n: number) => T; consume: (item: T) => void }): T;
declare function useNamed<T>(input: { 0: (n: number) => T; consume: (item: T) => void }): T;
declare const key: unique symbol;
declare function useSymbol<T>(input: { [key]: (n: number) => T; consume: (item: T) => void }): T;
const /*#method*/methodResult = use({ produce(n) { return "method" as const; }, consume(item) {} });
const /*#arrow*/arrowResult = use({ produce: n => "arrow" as const, consume: item => {} });
const /*#function*/functionResult = use({ produce: function(n) { return "function" as const; }, consume: function(item) {} });
const /*#computed*/computedResult = use({ ["produce"](n) { return "computed" as const; }, consume(item) {} });
const /*#numeric*/numericResult = useNamed({ 0(n) { return "numeric" as const; }, consume(item) {} });
const /*#symbol*/symbolResult = useSymbol({ [key](n) { return "symbol" as const; }, consume(item) {} });
const /*#annotated*/annotatedResult = use({ produce(n): "annotated" { return "annotated"; }, consume(item) {} });
const /*#consumerFirst*/consumerFirstResult = use({ consume(item) {}, produce(n) { return "later" as const; } });
const /*#mixed*/mixedResult = use({ produce(n) { return "mixed" as const; }, consume: item => {} });
const /*#missing*/missingResult = use({ produce(n) { return missingValue; }, consume(item) {} });
const /*#healthyAfter*/healthyAfterResult = use({ produce(n) { return "healthy" as const; }, consume(item) {} });
