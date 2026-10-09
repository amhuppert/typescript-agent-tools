// Augmentations through a nested alias, an alias of a shorthand module, a
// package whose earlier augmentation merged the table a later one reaches,
// and a pattern's alias.
export {};
declare module "./api" {
  namespace /*@N:d*/N { interface /*@X:d*/X { /*@!*/b: number } }
}
declare module "./short" {
  interface /*@Short:d*/Short { /*@!*/s: number }
}
declare module "thing/lib" {
  class /*@Thing:d*/Thing { /*@!*/c: number }
}
declare module "thing" {
  interface /*@Options:d*/Options { /*@!*/b: number }
}
declare module "x.css" {
  interface /*@Sheet:d*/Sheet { /*@!*/b: string }
}
