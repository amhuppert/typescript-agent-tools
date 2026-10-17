class Box<T> { value!: T; }
interface Box<T = string> { other:T; }
declare const result: /*#query*/Box;
