// An ambiguous star export is unresolved however it is reached; unique and
// shadowing exports, a diamond, and an `export *` cycle resolve.
import { /*@?*/shared, /*@onlyLeft:i*/onlyLeft, /*@onlyRight:i*/onlyRight } from "./both";
import * as /*@both:i*/both from "./both";
import { /*@local:i*/shared as /*@local:i*/local } from "./shadow";
import { /*@leftShared:i*/shared as /*@leftShared:i*/viaDiamond } from "./diamond";
import { /*@fromA:i*/fromA, /*@fromB:i*/fromB } from "./cycle-b";
import { /*@?*/looped } from "./loop-a";

/*@?*/shared; /*@onlyLeft:u*/onlyLeft; /*@onlyRight:u*/onlyRight();
/*@both:u*/both./*@?*/shared; /*@both:u*/both./*@onlyLeft:u*/onlyLeft;
/*@local:u*/local; /*@leftShared:u*/viaDiamond; /*@fromA:u*/fromA; /*@fromB:u*/fromB; /*@?*/looped;
export { /*@?*/shared as /*@?*/forwarded } from "./both";

// A `this` parameter and tuple member labels are not reference targets.
function /*@scale:d*/scale(/*@!*/this: { /*@!*/factor: number }, /*@value:d*/value: number) {
  return this./*@!*/factor * /*@value:u*/value;
}
type /*@Pair:d*/Pair = [/*@!*/first: number, /*@!*/second?: string, .../*@!*/rest: boolean[]];
let /*@pair:d*/pair: /*@Pair:u*/Pair = [1];
/*@scale:u*/scale; /*@pair:u*/pair;
