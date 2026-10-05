// Module and global augmentations merge with their targets.
import { /*@Options*/Options, /*@configure*/configure } from "./api";

declare module "./api" {
  interface /*@Options*/Options { /*@!*/extra: number }
}

declare /*@!*/global {
  interface /*@GlobalExtra*/GlobalExtra { /*@!*/flag: boolean }
}

let /*@merged*/merged: /*@Options*/Options = { /*@!*/base: 1, /*@!*/extra: 2 };
/*@configure*/configure(/*@merged*/merged);
