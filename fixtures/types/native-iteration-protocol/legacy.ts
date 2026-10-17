export {};
declare const numbers: number[];
const [/*#legacyArray*/legacyArray] = numbers;
declare const pair: [number, string];
const [/*#legacyTuple*/legacyTuple] = pair;
declare const text: string;
for (const /*#legacyString*/legacyString of text) {}
declare const maybeText: number[] | string;
for (const /*#legacyUnion*/legacyUnion of maybeText) {}
declare const absent: { other: number };
const [/*#legacyMissing*/legacyMissing] = absent;
declare const anyValue: any;
const [/*#legacyAny*/legacyAny] = anyValue;
