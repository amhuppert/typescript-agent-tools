export {};
declare const literalKey: "literal";
declare const symbolKey: unique symbol;
class Accessors {
 get [literalKey]() { return (/*#literal*/value) => value; }
 set [literalKey](value: (input: number) => number) {}
 get [symbolKey]() { return (/*#symbol*/value) => value; }
 set [symbolKey](value: (input: string) => string) {}
 get plain() { return (/*#plain*/value) => value; }
 set plain(value: (input: boolean) => boolean) {}
}
let healthy: number;
