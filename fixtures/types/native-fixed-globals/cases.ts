interface Box<T> { value: T; }
declare let /*#ordinary*/ordinary: Box<number>;
declare let /*#composite*/composite: Box<number> | Box<string>;
declare let /*#array*/array: number[];
declare let /*#readonly*/readonly: readonly number[];
const contextual: { value: number; method(): number } = {
    value: 1,
    method() { return /*#contextualThis*/this; }
};
export {};
