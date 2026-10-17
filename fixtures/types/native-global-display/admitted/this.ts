import type { MissingBase } from "./missing";
declare global {
  interface Iterable extends MissingBase { value: this }
}
interface Box<T> { value: T }
declare const /*#ordinary*/ordinary: Box<number>;
declare const /*#array*/array: number[];
declare const /*#nongeneric*/nongeneric: { value: number };
