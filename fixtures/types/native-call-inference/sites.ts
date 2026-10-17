import {identity, pair, make, numberConsumer, rejected, defaultsFirst, defaultsSkipped, receiver, unused} from "./definitions.js";
export const identityResult = identity("literal");
export const pairResult = pair("a", "b");
export const contextResult = numberConsumer(make());
export const rejectedResult = rejected(1);
export const eagerDefaultResult = defaultsFirst(1);
export const skippedDefaultResult = defaultsSkipped(1, "value");
export const receiverResult = receiver.method();
export const unusedResult = unused();
