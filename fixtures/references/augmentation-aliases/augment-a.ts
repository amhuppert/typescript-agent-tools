// Augmentations through each export form.
import type { /*@Commands:i*/Commands } from "./commands";
declare module "./commands" {
  interface /*@Commands:d*/Commands</*@T:d*/T> { /*@!*/a: /*@T:u*/T }
}
declare module "./options" {
  interface /*@Options:d*/Options { /*@!*/b: number }
}
declare module "./renamed" {
  interface /*@Renamed:d*/Renamed</*@U:d*/U> { /*@!*/extra: /*@U:u*/U }
}
// A package that re-exports from other modules, re-exports an import, and
// supplies a name through `export *`.
declare module "kit" {
  interface /*@Base:d*/Base { /*@!*/c: number }
  interface /*@Widget:d*/Widget { /*@!*/x: number }
  interface /*@Starred:d*/Starred</*@V:d*/V> { /*@!*/y: /*@V:u*/V }
}
declare module "./shape" {
  class /*@Shape:d*/Shape { /*@!*/t: number }
}
declare module "./mode" {
  interface /*@ModeType:d*/Mode { /*@!*/m: number }
}
export declare const /*@c:de*/c: /*@Commands:u*/Commands<number>;
