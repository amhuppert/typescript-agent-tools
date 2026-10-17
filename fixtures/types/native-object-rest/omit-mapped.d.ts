type Exclude<T, U> = T extends U ? never : T;
type Omit<T, K> = { [P in Exclude<keyof T, K>]: T[P] };
