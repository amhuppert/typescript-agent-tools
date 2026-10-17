export {};
declare const thenNumber: { then(callback: (value: number) => unknown): unknown };
const /*#checked*/checked = (() => 1) satisfies (() => string);
const /*#quiet*/quiet = await thenNumber;
const /*#compatible*/compatible = (() => 1) satisfies (() => number);
const /*#quietThenChecked*/quietThenChecked = (await thenNumber, (() => 1) satisfies (() => string));
