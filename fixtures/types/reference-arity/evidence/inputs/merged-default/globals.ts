interface Box<T> { value: T; }
interface Box<T = number> {}
declare const result: /*#query*/Box;
