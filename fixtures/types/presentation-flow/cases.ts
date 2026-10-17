/// <reference path="./dependency.ts" />
interface Array<T> { length: number; [index: number]: typeof dependency.value }
declare const array: Array<number>;
interface Child { (value: string): string; [key: string]: typeof dependency.value }
interface Host { nested: Child; direct: typeof dependency.value }
declare const host: Host;
type Flowed = typeof dependency.value;

declare const laterInput: string | number;
class LaterFlowSource {
  get value() {
    if (typeof laterInput === "string") return laterInput;
    return 0;
  }
}
declare const laterDependency: LaterFlowSource;
interface Shared { get value(): typeof dependency.value; set value(input: typeof dependency.value); }
interface Independent { get value(): typeof dependency.value; set value(input: typeof laterDependency.value); }
declare const shared: Shared;
declare const independent: Independent;
declare const later: typeof laterDependency.value;
