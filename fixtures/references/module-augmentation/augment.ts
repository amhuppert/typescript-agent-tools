// Module and global augmentations merge with their targets.
import { /*@Options:i*/Options, /*@configure:i*/configure } from "./api";

declare module "./api" {
  interface /*@Options:d*/Options { /*@!*/extra: number }
}

declare /*@!*/global {
  interface /*@GlobalExtra:d*/GlobalExtra { /*@!*/flag: boolean }
}

let /*@merged:d*/merged: /*@Options:u*/Options = { /*@!*/base: 1, /*@!*/extra: 2 };
/*@configure:u*/configure(/*@merged:u*/merged);
