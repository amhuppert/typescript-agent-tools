export declare function identity<T>(value: T): T;
export declare function pair<T>(first: T, second: T): T;
export declare function make<T>(): T;
export declare function numberConsumer(value: number): "context";
export declare function rejected<T extends string>(value: T): "string";
export declare function rejected(value: number): "number";
export declare function defaultsFirst<T = MissingEagerDefault>(value: T): T;
export declare function defaultsSkipped<T, U = MissingSkippedDefault>(first: T, second: U): "skipped";
export interface Receiver { label: "receiver"; method: <T>(this: T) => T; }
export declare const receiver: Receiver;
export declare function unused<T>(unused?: MissingUnusedArgument): T;
