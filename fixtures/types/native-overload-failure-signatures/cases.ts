export {};

declare function separate(value: string): "text";
declare function separate(value: number): "number";
const /*#separateReturns*/separateReturns = separate(true);

declare function objectResults(value: string): { left: string };
declare function objectResults(value: number): { right: number };
const /*#intersectedReturns*/intersectedReturns = objectResults(true);

declare function actualAny(value: string): any;
declare function actualAny(value: number): number;
const /*#genuineAnyReturn*/genuineAnyReturn = actualAny(true);

declare function selected(value: string): "selected";
declare function selected(value: number): MissingUnchosenReturn;
const /*#selectedSkipsReturn*/selectedSkipsReturn = selected("ok");

declare function allReturns(value: string): MissingFirstReturn;
declare function allReturns(value: number): MissingSecondReturn;
const /*#allMissingReturns*/allMissingReturns = allReturns(true);

declare function allPositions(value: string, later?: MissingFirstParameter): MissingPositionReturnA;
declare function allPositions(value: number, later?: MissingSecondParameter): MissingPositionReturnB;
const /*#positionsBeforeReturns*/positionsBeforeReturns = allPositions(true);

declare function uneven(value: string): { short: true };
declare function uneven(value: number, second?: MissingExtraPosition, third?: boolean): { long: true };
const /*#unevenRawCounts*/unevenRawCounts = uneven(true);

declare function fixedTuple(value: string, ...tail: [count: number, enabled?: boolean]): { tuple: true };
declare function fixedTuple(value: number, fallback: string): { fixed: true };
const /*#fixedTupleRest*/fixedTupleRest = fixedTuple(true, 1);

declare function variadicTuple(value: string, ...tail: [head: number, ...rest: boolean[]]): { variadic: true };
declare function variadicTuple(value: number, extra: string, ...rest: number[]): { array: true };
const /*#variadicTupleRest*/variadicTupleRest = variadicTuple(true, 1, false);

declare function missingRest(value: string, ...tail: MissingRestElement[]): MissingRestReturnA;
declare function missingRest(value: number, ...tail: boolean[]): MissingRestReturnB;
const /*#restBeforeReturns*/restBeforeReturns = missingRest(true);

declare function emptyTuples(...left: [value: string]): "left";
declare function emptyTuples(...right: [value: number]): "right";
const /*#tupleOnlyRawZero*/tupleOnlyRawZero = emptyTuples(true);

declare function optionalNames(first: string, optionalFirst?: number): { optional: true };
declare function optionalNames(second: number, requiredSecond: boolean): { required: true };
const /*#optionalFirstSource*/optionalFirstSource = optionalNames(true, true);

declare const receiver: {
  method(this: { tag: "first" }, value: string): { first: true };
  method(this: { tag: "second" }, value: number): { second: true };
};
const /*#combinedThis*/combinedThis = receiver.method(true);

declare const missingReceiver: {
  method(this: MissingFirstThis, value: string): MissingThisReturnA;
  method(this: MissingSecondThis, value: number): MissingThisReturnB;
};
const /*#thisBeforeReturns*/thisBeforeReturns = missingReceiver.method(true);

declare function specialized(value: string): { broad: true };
declare function specialized(value: "literal"): { literal: true };
declare function specialized(value: number): { numeric: true };
const /*#literalCandidateFlags*/literalCandidateFlags = specialized(true);

declare function callback(value: string, handler: (first: number) => void): 0;
declare function callback(value: number, handler: (second: string) => void): 0;
callback(true, (/*#combinedCallbackParameter*/input) => {});

declare function tupleCallback(...args: [value: string, handler: (tupleInput: number) => void]): 0;
declare function tupleCallback(value: number, handler: (ordinaryInput: string) => void): 0;
tupleCallback(true, (/*#tupleCallbackParameter*/input) => {});

declare function single(value: string): number;
const /*#singleCandidateControl*/singleCandidateControl = single(true);

declare function generic<T>(value: string): T;
declare function generic(value: number): MissingGenericSkippedReturn;
const /*#genericLongestControl*/genericLongestControl = generic<boolean>(true);

const /*#healthyIndependent*/healthyIndependent = 42;
