export {};
namespace ObjectOnly {
  type Identity<T> = T;
  type Flatten<T> = Identity<{ [K in keyof T]: T[K] }>;
  type Inner<T, U = string> = T extends object ? Flatten<{ [K in keyof T]?: Formatted<T[K], U> }> : any;
  type Formatted<T, U = string> = { _errors: U[] } & Flatten<Inner<T, U>>;
  interface Failure<T> { format(): Formatted<T>; format<U>(mapper: (issue: unknown) => U): Formatted<T, U>; }
  type Output<T> = T extends { output: unknown } ? T["output"] : never;
  type Result<T> = { success: true; data: T } | { success: false; error: Failure<T> };
  interface Schema<out T = unknown> {
    output: T;
    parse(): Output<this>;
    safeParse(): Result<Output<this>>;
  }
  interface Concrete<T> extends Schema<T> { shape: T; }
  declare function make<T>(value: T): Concrete<T>;
  declare function register<T extends Schema>(schema: T): T;
  const registered = register(make({ id: "text" }));
  const /*#ObjectOnly*/contextualResult = registered.parse().id;
}
namespace NoFlatten {
  type Identity<T> = T;
  type Flatten<T> = Identity<{ [K in keyof T]: T[K] }>;
  type Inner<T, U = string> = T extends object ? { [K in keyof T]?: Formatted<T[K], U> } : any;
  type Formatted<T, U = string> = { _errors: U[] } & Flatten<Inner<T, U>>;
  interface Failure<T> { format(): Formatted<T>; format<U>(mapper: (issue: unknown) => U): Formatted<T, U>; }
  type Output<T> = T extends { output: unknown } ? T["output"] : never;
  type Result<T> = { success: true; data: T } | { success: false; error: Failure<T> };
  interface Schema<out T = unknown> {
    output: T;
    parse(): Output<this>;
    safeParse(): Result<Output<this>>;
  }
  interface Concrete<T> extends Schema<T> { shape: T; }
  declare function make<T>(value: T): Concrete<T>;
  declare function register<T extends Schema>(schema: T): T;
  const registered = register(make({ id: "text" }));
  const /*#NoFlatten*/contextualResult = registered.parse().id;
}
namespace Nonrecursive {
  type Identity<T> = T;
  type Flatten<T> = Identity<{ [K in keyof T]: T[K] }>;
  type Inner<T, U = string> = T extends object ? Flatten<{ [K in keyof T]?: U }> : any;
  type Formatted<T, U = string> = { _errors: U[] } & Flatten<Inner<T, U>>;
  interface Failure<T> { format(): Formatted<T>; format<U>(mapper: (issue: unknown) => U): Formatted<T, U>; }
  type Output<T> = T extends { output: unknown } ? T["output"] : never;
  type Result<T> = { success: true; data: T } | { success: false; error: Failure<T> };
  interface Schema<out T = unknown> {
    output: T;
    parse(): Output<this>;
    safeParse(): Result<Output<this>>;
  }
  interface Concrete<T> extends Schema<T> { shape: T; }
  declare function make<T>(value: T): Concrete<T>;
  declare function register<T extends Schema>(schema: T): T;
  const registered = register(make({ id: "text" }));
  const /*#Nonrecursive*/contextualResult = registered.parse().id;
}
