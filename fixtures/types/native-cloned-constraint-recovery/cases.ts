export {};
declare const later: { value: /*@later*/MissingLaterClone };
interface Circular<T> { <U extends /*@circular*/U>(first: U, tail: number): T; }
interface Mutual<T> { <U extends /*@mutualU*/V, V extends /*@mutualV*/U>(first: U, second: V, tail: number): T; }
interface Missing<T> { <U extends /*@missing*/MissingClonedConstraint>(first: U, tail: number): T; }
interface Unconstrained<T> { <U>(first: U, tail: number): T; }
interface Bound<T> { <U extends T>(first: U, tail: number): T; }
declare const circular: Circular<number>;
declare const mutual: Mutual<number>;
declare const missing: Missing<number>;
declare const unconstrained: Unconstrained<number>;
declare const bound: Bound<number>;
const /*#circularTyped*/circularTyped = circular<number>(1, later.value);
const /*#circularInferred*/circularInferred = circular(1, later.value);
const /*#circularWithoutTailFailure*/circularWithoutTailFailure = circular<number>(1, 2);
const /*#mutualTyped*/mutualTyped = mutual<number, string>(1, "x", later.value);
const /*#missingTyped*/missingTyped = missing<number>(1, later.value);
const /*#unconstrainedLater*/unconstrainedLater = unconstrained<number>(1, later.value);
const /*#mappedBoundLater*/mappedBoundLater = bound<number>(1, later.value);
const /*#unconstrainedHealthy*/unconstrainedHealthy = unconstrained<number>(1, 2);
const /*#mappedBoundHealthy*/mappedBoundHealthy = bound<number>(1, 2);
const /*#healthyAfter*/healthyAfter = 1 + 2;
