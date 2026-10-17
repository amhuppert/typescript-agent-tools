interface Weak { foo?: string; }
interface Callable { (): MissingCallReturn; }
interface Constructable { new (): MissingConstructReturn; }
interface Both { (): { foo: string }; new (): MissingSkippedConstruct; }
interface Second { (): { other: number }; new (): MissingSecondConstruct; }
declare function weak<T extends Weak>(): "matched";
declare function weak<T>(): "fallback";
const /*#weakCallSilent*/weakCallSilent = weak<Callable>();
const /*#weakCallReport*/weakCallReport = weak<Callable>;
const /*#weakConstructSilent*/weakConstructSilent = weak<Constructable>();
const /*#weakConstructReport*/weakConstructReport = weak<Constructable>;
const /*#weakBothSilent*/weakBothSilent = weak<Both>();
const /*#weakBothReport*/weakBothReport = weak<Both>;
const /*#weakSecondSilent*/weakSecondSilent = weak<Second>();
const /*#weakSecondReport*/weakSecondReport = weak<Second>;

interface Actual {
  (x: { a: number; hidden?: MissingFirstParameter }): void;
  (x: { a: string }): void;
}
declare function overload<T extends (x: { a: string }) => void>(): "matched";
const /*#overloadSilent*/overloadSilent = overload<Actual>();
const /*#overloadReport*/overloadReport = overload<Actual>;

interface Bad { a: { p: 0; hidden?: MissingCachedProperty }; }
interface L { a: { p: 1 }; }
interface R { other: boolean; }
declare function union<T extends L | R>(): "matched";
declare function union<T>(): "fallback";
const /*#cacheSilent*/cacheSilent = union<Bad>();
const /*#cacheReport*/cacheReport = union<Bad>;

declare function literal<T extends string>(): "matched";
const /*#rejected*/rejected = literal<number>;
const /*#accepted*/accepted = literal<"value">;
declare function tuple<T extends [number, string]>(): "matched";
const /*#tupleReadonly*/tupleReadonly = tuple<readonly [number, string]>;
const /*#tupleShort*/tupleShort = tuple<[number]>;
const /*#tupleLong*/tupleLong = tuple<[number, string, boolean]>;
declare function index<T extends { [key: string]: number }>(): "matched";
const /*#indexProperty*/indexProperty = index<{ item: string }>;
const /*#indexKey*/indexKey = index<{ [key: number]: string }>;
declare function optional<T extends { value: string }>(): "matched";
const /*#optional*/optionalValue = optional<{ value?: string }>;
declare function missingOne<T extends { a: number }>(): "matched";
declare function missingFive<T extends { a: number; b: number; c: number; d: number; e: number }>(): "matched";
declare function missingSix<T extends { a: number; b: number; c: number; d: number; e: number; f: number }>(): "matched";
const /*#missingOne*/one = missingOne<{}>;
const /*#missingFive*/five = missingFive<{}>;
const /*#missingSix*/six = missingSix<{}>;
declare class Base { private value: number; }
declare class Other { private value: number; }
declare function nominal<T extends Base>(): "matched";
const /*#private*/privateValue = nominal<Other>;
