export {};
type Copy<T> = { [P in keyof T]: T[P] };
type Named<T> = { [P in keyof T as P]: T[P] };
type MutableRequired<T> = { -readonly [P in keyof T]-?: T[P] };
type Add<T> = { readonly [P in keyof T]?: T[P] };
declare const /*#primitive*/primitive: Copy<string>;
declare const /*#union*/union: Copy<{ left: string } | { right: number }>;
declare const /*#array*/array: Copy<string[]>;
declare const /*#readonlyArray*/readonlyArray: Copy<readonly string[]>;
declare const /*#tuple*/tuple: Copy<readonly [first: string, second?: number, ...tail: boolean[]]>;
declare const /*#mutableTuple*/mutableTuple: MutableRequired<readonly [first: string, second?: number]>;
declare const /*#optionalTuple*/optionalTuple: Add<[first: string, second: number]>;
declare const /*#namedTuple*/namedTuple: Named<readonly [first: string, second?: number]>;
declare const /*#namedArray*/namedArray: Named<readonly string[]>;
declare const /*#intersection*/intersection: Copy<string[] & readonly number[]>;
declare const /*#any*/anyValue: Copy<any>;
declare const /*#unknown*/unknownValue: Copy<unknown>;
declare const /*#never*/neverValue: Copy<never>;
function variadic<T extends unknown[]>() { let /*#variadic*/value: Copy<[first: string, ...tail: T]>; return value!; }
