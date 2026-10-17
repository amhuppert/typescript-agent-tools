interface First<T extends MissingConstraint = MissingDefault> { value: T; }
interface Second<T extends number = 1> { value: T; }
interface Joined<U> { first: U; }
interface Joined<U extends string = "ready"> { second: U; }
interface Escaped<\u0056 extends string = "done"> { value: \u0056; }
interface Escaped<V> { other: V; }
interface PlainFirst<W extends string = "plain"> { first: W; }
interface PlainFirst<\u0057> { second: \u0057; }
