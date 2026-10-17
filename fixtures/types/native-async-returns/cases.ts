export {};
declare const promise: Promise<number>;
declare const nested: Promise<Promise<number>>;
declare const structural: { then(onfulfilled: (value: string) => unknown): unknown };
declare const invalid: { then(): void };
declare const anyValue: any;
declare const missing: { value: /*@missingValue*/MissingReturnValue };
declare const secondMissing: { value: /*@secondValue*/MissingSecondReturn };
async function /*#scalar*/scalar(value: number) { return value; }
async function /*#direct*/direct() { return promise; }
async function /*#nested*/nestedReturn() { return nested; }
async function /*#structural*/structuralReturn() { return structural; }
async function /*#any*/anyReturn() { return anyValue; }
async function /*#empty*/emptyReturn() {}
async function /*#bare*/bareReturn() { return; }
async function /*#self*/selfReturn() { return selfReturn(); }
async function /*#awaitSelf*/awaitSelfReturn() { return await awaitSelfReturn(); }
const /*#arrow*/arrow = async () => promise;
const /*#literal*/literal = async () => 1;
async function /*#invalid*/invalidReturn() { return invalid; }
const /*#invalidArrow*/invalidArrow = async () => invalid;
async function /*#missing*/missingReturn() { return missing.value; }
async function /*#later*/laterReturn(condition: boolean) {
    if (condition) return missing.value;
    return secondMissing.value;
}
async function /*#invalidLater*/invalidLater(condition: boolean) {
    if (condition) return invalid;
    return missing.value;
}
async function /*#annotated*/annotated(): Promise<number> { return missing.value; }
declare function acceptsLiteral<T>(fn: () => Promise<T>): T;
const /*#contextual*/contextual = acceptsLiteral(async () => 1);
declare function acceptsEmpty<T>(fn: (unused: T) => Promise<undefined>): T;
const /*#contextualEmpty*/contextualEmpty = acceptsEmpty(async unused => {});
function /*#nullWidening*/nullWidening() { return { value: null }; }
function /*#siblingWidening*/siblingWidening(condition: boolean) {
    if (condition) return { first: null };
    return { second: undefined };
}
function /*#wideningLazy*/wideningLazy() { return { value: null, get tail(): /*@lazyTail*/MissingLazyTail { return anyValue; } }; }
