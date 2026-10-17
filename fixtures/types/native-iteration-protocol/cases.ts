export {};
declare const anyInput: any;
const [/*#actualAny*/actualAny] = anyInput;
declare const fast: Iterable<number, string, unknown>;
const [/*#fastIterable*/fastIterable] = fast;
declare const objectFast: IteratorObject<string, number, unknown>;
const [/*#iteratorObject*/iteratorObject] = objectFast;
declare const builtin: ArrayIterator<boolean>;
const [/*#builtin*/builtinResult] = builtin;
declare const arrayInput: number[];
const [/*#array*/arrayResult] = arrayInput;
declare const tupleInput: [number, string, boolean?];
const [/*#tuple*/tupleResult, ...tupleRest] = tupleInput;
declare const structural: { iter(): { next(): { value: number; done?: false } } };
const [/*#structural*/structuralResult] = structural;
declare const both: { iter(): { next(): { value: number; done: false } | { value: string; done: true } } };
const [/*#doneUnion*/doneUnion] = both;
declare const nextNumber: { iter(): { next(value: number): { value: boolean } } };
const [/*#nextInput*/nextInput] = nextNumber;
declare const overloads: { iter(value: string): { next(): { value: string } }; iter(): { next(): { value: number } } };
const [/*#overloads*/overloadResult] = overloads;
declare const requiresArgument: { iter(value: string): { next(): { value: number } } };
const [/*#requiredArgument*/requiredArgument] = requiresArgument;
declare const optionalIterator: { iter?(): { next(): { value: number } } };
const [/*#optionalIterator*/optionalIteratorResult] = optionalIterator;
declare const absentIterator: { unrelated: number };
const [/*#missingMethod*/missingMethod] = absentIterator;
declare const invalidResult: { iter(): { next(): {} } };
const [/*#invalidResult*/invalidResultValue] = invalidResult;
declare const optionalNext: { iter(): { next?(): { value: number } } };
const [/*#optionalNext*/optionalNextValue] = optionalNext;
declare const badMethods: { iter(): { next(): MissingNextResult; return(): MissingReturnResult; throw(): MissingThrowResult } };
const [/*#orderedMethods*/orderedMethods] = badMethods;
declare const badDone: { iter(): { next(): { done: MissingDone; value: number } } };
const [/*#doneRecovery*/doneRecovery] = badDone;
declare const badValues: { iter(): { next(): { done: false; value: MissingYieldValue } | { done: true; value: MissingReturnValue } } };
const [/*#valueRecovery*/valueRecovery] = badValues;
declare const healthyUnion: Iterable<number> | Iterable<string>;
const [/*#union*/unionResult] = healthyUnion;
declare const brokenUnion: Iterable<number> | { unrelated: number };
const [/*#unionMissing*/unionMissing] = brokenUnion;
declare const neverInput: never;
const [/*#never*/neverResult] = neverInput;
declare const asyncInput: AsyncIterable<Promise<number>>;
for await (const /*#asyncFast*/asyncFast of asyncInput) {}
declare const syncPromises: Iterable<Promise<string>>;
for await (const /*#asyncFromSync*/asyncFromSync of syncPromises) {}
declare const asyncStructural: { asyncIter(): { next(): Promise<{ value: boolean; done: false }> } };
for await (const /*#asyncSlow*/asyncSlow of asyncStructural) {}
const [/*#after*/after] = fast;
declare const yieldOnly: { iter(): { next(): IteratorYieldResult<number> } };
const [/*#onlyYield*/onlyYield] = yieldOnly;
declare const returnsOnly: { iter(): { next(): IteratorReturnResult<number> } };
for await (const /*#asyncOnlyReturn*/asyncOnlyReturn of returnsOnly) {}
declare const badAsyncThen: AsyncIterable<{ then(callback: number): unknown }>;
for await (const /*#asyncBadThen*/asyncBadThen of badAsyncThen) {}
declare const badAsyncResult: { asyncIter(): { next(): { then(callback: number): unknown } } };
for await (const /*#asyncBadReturn*/asyncBadReturn of badAsyncResult) {}
