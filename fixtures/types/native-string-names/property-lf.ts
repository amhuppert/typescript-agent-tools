export {};
const keyed = { "\uD800\
\uDC00": 1 };
declare const /*#result*/nameIsScalar: keyof typeof keyed extends "𐀀" ? true : false;
