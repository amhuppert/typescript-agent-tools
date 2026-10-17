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

declare function properties<T>(input: { produce: (n: number) => MissingProduce; consume: (item: MissingConsume) => void }, tag: T): T;
const /*#properties*/propertiesResult = properties({ produce(n) { return 0; }, consume(item) {} }, 41 as const);
declare function computed<T>(input: { produce: (n: number) => T; consume: (item: T) => void }): T;
const /*#computed*/computedResult = computed({ [missingName](n) { return missingReturn; }, consume(item) {} });
declare function indexed<T>(input: { [name: string]: (n: MissingIndexParameter) => MissingIndexReturn }, tag: T): T;
const /*#indexed*/indexedResult = indexed({ first(n) { return 0; }, second(n) { return 0; } }, 42 as const);
type Copy<T> = { [P in keyof T]: T[P] };
declare function mapped<T>(input: Copy<{ produce: (n: number) => MissingMappedProduce; consume: (item: MissingMappedConsume) => void }>, tag: T): T;
const /*#mapped*/mappedResult = mapped({ produce(n) { return 0; }, consume(item) {} }, 43 as const);
declare function healthy(input: { produce: (n: number) => number; consume: (item: number) => void }): 44;
const /*#healthy*/healthyResult = healthy({ produce(n) { return 0; }, consume(item) {} });
