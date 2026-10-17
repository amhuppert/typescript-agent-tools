export {};
declare function wrapper<T>(value: T): T;
declare function m(cb: (x: string) => number): 'first';
declare function m(cb: (x: any) => string): 'second';
const /*#historyResult*/result = wrapper(m(/*#historyParameter*/x => x.toUpperCase()));
