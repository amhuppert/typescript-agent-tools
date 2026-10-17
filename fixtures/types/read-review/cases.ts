export {};
let uninitialized: number;
/*#uninitialized*/uninitialized;
type UninitializedType = typeof /*#typeQuery*/uninitialized;
/*#nonNull*/uninitialized!;
let count: number = 1;
if (typeof Missing === 'string') { /*#unrelatedMissing*/count; }
interface OnlyType {}
if (typeof OnlyType === 'string') { /*#unrelatedType*/count; }
declare const neverValue: never;
if (typeof Missing === 'string') { /*#neverMissing*/neverValue; }
