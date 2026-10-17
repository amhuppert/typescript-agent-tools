export {};
declare const direct: Promise<number>;
declare const nested: Promise<Promise<string>>;
declare const structural: { then(onfulfilled: (value: boolean) => unknown): unknown };
declare const absent: { value: number };
declare const nonCallable: { then: number };
declare const anyThen: { then: any };
declare const optionalThen: { then?: (onfulfilled: (value: string) => unknown) => unknown };
declare const voidThis: { then(this: void, onfulfilled: (value: number) => unknown): unknown };
declare const matchingThis: { tag: number; then(this: { tag: number }, onfulfilled: (value: number) => unknown): unknown };
declare const wrongThis: { then(this: { absent: number }, onfulfilled: (value: number) => unknown): unknown };
declare const hiddenThis: {
    receiver: /*@receiverDisplay*/MissingReceiverDisplay;
    then(this: { absent: /*@thisDisplay*/MissingThisDisplay }, onfulfilled: (value: number) => unknown): unknown;
};
declare const anyCallback: { then(onfulfilled: any): unknown };
declare const noCallback: { then(): unknown };
declare const noValue: { then(onfulfilled: () => unknown): unknown };
declare const missingCallback: { then(onfulfilled: /*@callback*/MissingCallback): unknown };
declare const missingValue: { then(onfulfilled: (value: /*@fulfilled*/MissingFulfilled) => unknown): unknown };
interface ThenOverloads {
    then(onfulfilled: (value: number) => unknown): unknown;
    then(onfulfilled: (value: string) => unknown): unknown;
}
declare const overloadedThen: ThenOverloads;
interface MixedThisOverloads {
    then(this: { absent: MissingSkippedThis }, onfulfilled: (value: MissingSkippedCallback) => unknown): unknown;
    then(this: void, onfulfilled: (value: number) => unknown): unknown;
}
declare const mixedThis: MixedThisOverloads;
interface CallbackOverloads { (value: number): unknown; (value: string): unknown; }
declare const overloadedCallback: { then(onfulfilled: CallbackOverloads): unknown };
interface SelfThenable { then(onfulfilled: (value: SelfThenable) => unknown): unknown; }
interface MutualA { then(onfulfilled: (value: MutualB) => unknown): unknown; }
interface MutualB { then(onfulfilled: (value: MutualA) => unknown): unknown; }
declare const self: SelfThenable;
declare const mutual: MutualA;
declare const nilThenMissing: { then(): unknown } | { then(onfulfilled: (value: /*@unionValue*/MissingUnionValue) => unknown): unknown };
declare const allNil: SelfThenable | { then(): unknown };
declare const invocation: {
    then(onfulfilled: (value: /*@invocationValue*/MissingInvocationValue) => unknown): unknown;
    tail: /*@invocationDisplay*/MissingInvocationDisplay;
};
declare const invocationUnion: typeof invocation | { nonPromise: true };
const /*#directPromise*/directPromise = await direct;
const /*#nestedPromise*/nestedPromise = await nested;
const /*#structuralThen*/structuralThen = await structural;
const /*#absentThen*/absentThen = await absent;
const /*#nonCallableThen*/nonCallableThen = await nonCallable;
const /*#anyThenType*/anyThenType = await anyThen;
const /*#optionalThenType*/optionalThenType = await optionalThen;
const /*#voidThisType*/voidThisType = await voidThis;
const /*#matchingThisType*/matchingThisType = await matchingThis;
const /*#wrongThisType*/wrongThisType = await wrongThis;
const /*#hiddenThisTypes*/hiddenThisTypes = await hiddenThis;
const /*#anyCallbackType*/anyCallbackType = await anyCallback;
const /*#noCallbackType*/noCallbackType = await noCallback;
const /*#noValueType*/noValueType = await noValue;
const /*#missingCallbackType*/missingCallbackType = await missingCallback;
const /*#missingFulfilledType*/missingFulfilledType = await missingValue;
const /*#overloadedThenType*/overloadedThenType = await overloadedThen;
const /*#mixedThisType*/mixedThisType = await mixedThis;
const /*#overloadedCallbackType*/overloadedCallbackType = await overloadedCallback;
const /*#selfRecursive*/selfRecursive = await self;
const /*#mutualRecursive*/mutualRecursive = await mutual;
const /*#unionNilThenMissing*/unionNilThenMissing = await nilThenMissing;
const /*#unionAllNil*/unionAllNil = await allNil;
const /*#quickAwaitAssertion*/quickAwaitAssertion = await (/*@quickOperand*/MissingQuickOperand as Promise</*@quickValue*/MissingQuickValue>);
const /*#checkedAwaitAssertion*/checkedAwaitAssertion = (await (/*@checkedOperand*/MissingCheckedOperand as Promise</*@checkedValue*/MissingCheckedValue>)) satisfies unknown;
// With this fixture's empty Function interface, the object call can take
// isUntypedFunctionCall. Its clean Any does not prove a quick-call bypass.
const /*#invocationMissingAwait*/invocationMissingAwait = invocation();
// A union excludes that untyped fallback. Both alternatives have no call or
// construct signature, so checked resolution reaches invocationErrorDetails.
const /*#checkedInvocationMissingAwait*/checkedInvocationMissingAwait = invocationUnion() satisfies unknown;
const /*#repeatedAwait*/repeatedAwait = (await missingValue) + (await missingValue);
const /*#healthyAfter*/healthyAfter = 1 + 2;
