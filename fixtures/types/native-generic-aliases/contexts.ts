interface Carrier<A, B extends A> { B(): number }
declare let /*#methods*/methods: Carrier<string, "shared"> | Carrier<string | number, "shared">;
type Callable<T> = (value: T) => T;
declare let /*#callable*/callable: Callable<string>;
type Constructor<T> = new (value: T) => { value: T };
declare let /*#constructor*/constructor: Constructor<number>;
