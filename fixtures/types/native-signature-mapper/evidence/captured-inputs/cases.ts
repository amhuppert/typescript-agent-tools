interface Simple<T> {
  (this: T, input: T): T;
  new(input: T): T;
}
declare let simple: Simple<string>;

interface Generic<T> {
  <U extends T = T>(input: U): U;
}
declare let genericString: Generic<string>;
declare let genericNumber: Generic<number>;

interface Nested<T> {
  <U extends T, V extends U = U>(first: U, second: V): V;
}
declare let nested: Nested<number>;

interface Shadow<T> {
  <T extends string>(input: T): T;
}
declare let shadow: Shadow<number>;

interface Lazy<T> {
  <U extends MissingConstraint = MissingDefault>(input: U): T;
}
declare let lazy: Lazy<string>;
