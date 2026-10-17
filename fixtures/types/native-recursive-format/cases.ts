export {};
type Identity<T> = T;
type Flatten<T> = Identity<{ [K in keyof T]: T[K] }>;
type Inner<T, U = string> = T extends [any, ...any[]] ? {
    [K in keyof T]?: Formatted<T[K], U>;
} : T extends any[] ? {
    [k: number]: Formatted<T[number], U>;
} : T extends object ? Flatten<{
    [K in keyof T]?: Formatted<T[K], U>;
}> : any;
type Formatted<T, U = string> = { _errors: U[] } & Flatten<Inner<T, U>>;
interface Failure<T> { format(): Formatted<T>; format<U>(mapper: (issue: unknown) => U): Formatted<T, U>; }
declare const objectError: Failure<{value:string}>;
const /*#object*/objectResult = objectError.format()._errors;
declare const nestedError: Failure<{value:{nested:string}}>;
const /*#nested*/nestedResult = nestedError.format()._errors;
declare const arrayError: Failure<string[]>;
const /*#array*/arrayResult = arrayError.format()._errors;
declare const tupleError: Failure<[string,number]>;
const /*#tuple*/tupleResult = tupleError.format()._errors;
declare const anyError: Failure<any>;
const /*#any*/anyResult = anyError.format()._errors;
declare const unknownError: Failure<unknown>;
const /*#unknown*/unknownResult = unknownError.format()._errors;
declare const primitiveError: Failure<string>;
const /*#primitive*/primitiveResult = primitiveError.format()._errors;
function inspect<T>(failure: Failure<T>) { const /*#generic*/genericResult = failure.format()._errors; }
declare function accept(failure: Failure<unknown>): number;
const /*#relation*/relationResult = accept(objectError);
function infer<T>(failure: Failure<T>) { const /*#relationGeneric*/genericRelationResult = accept(failure); }

type Output<T> = T extends { output: unknown } ? T["output"] : never;
type Result<T> = { success: true; data: T } | { success: false; error: Failure<T> };
interface Schema<out T = unknown> {
  output: T;
  parse(): Output<this>;
  safeParse(): Result<Output<this>>;
}
interface Concrete<T> extends Schema<T> { shape: T; }
declare function make<T>(value: T): Concrete<T>;
declare function register<T extends Schema>(schema: T): T;
const registered = register(make({ id: "text" }));
const /*#contextual*/contextualResult = registered.parse().id;
