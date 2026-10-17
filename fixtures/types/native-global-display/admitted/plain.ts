import type { MissingBase } from "./missing";
declare global {
  interface Iterable {}
}
interface Box<T> { value: T }
declare const /*#ordinary*/ordinary: Box<number>;
declare const /*#array*/array: number[];
declare const /*#nongeneric*/nongeneric: { value: number };
