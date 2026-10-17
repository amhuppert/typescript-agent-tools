// These declarations retain the exact bundled lib.es5.d.ts definitions.
type Pick<T, K extends keyof T> = { [P in K]: T[P] };
type Exclude<T, U> = T extends U ? never : T;
type Omit<T, K extends keyof any> = Pick<T, Exclude<keyof T, K>>;
// Independent ordinary names prove that the implementation has no name dispatch.
type SelectFields<T, K extends keyof T> = { [P in K]: T[P] };
type RemoveKeys<T, U> = T extends U ? never : T;
type ExceptFields<T, K extends keyof any> = SelectFields<T, RemoveKeys<keyof T, K>>;
type Capitalize<S extends string> = intrinsic;
