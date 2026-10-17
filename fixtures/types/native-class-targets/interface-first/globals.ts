interface Box<T> { other:T; }
class Box<T> { value!:T; }
declare const result: /*#query*/Box<number>;
