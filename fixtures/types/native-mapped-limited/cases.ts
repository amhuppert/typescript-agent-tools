export {};
declare function limited<T>(value: { [P in keyof T & ('a' | 'b')]: { value: T[P] } }): T;
const /*#healthy*/healthy = limited({ a: { value: 'a' }, b: { value: 1 } });
const /*#rejected*/rejected = limited({ a: { value: 'a' }, b: { value: 1 }, extra: { value: false } });
