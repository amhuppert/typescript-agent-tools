declare const external: string | number;
class C {
  get value() {
    if (typeof external === "string") return external;
    return 0;
  }
}
declare const c: C;
type Box<T extends typeof c.value> = T;
type R<T> = T extends Box<infer /*#query*/U> ? U : never;
