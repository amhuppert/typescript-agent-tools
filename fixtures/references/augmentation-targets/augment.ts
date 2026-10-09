// Augmentations of a name a star export supplies, of an `export =`
// namespace, of the module a namespace export names, and of one specifier
// that a pattern ambient module matches.
export {};
declare module "./barrel" {
  interface /*@Starred:d*/Starred { /*@!*/t: number }
}
declare module "./legacy" {
  interface /*@Settings:d*/Settings { /*@!*/b: number }
}
declare module "./api" {
  namespace /*@ns:d*/ns { interface /*@Extra:d*/Extra { /*@!*/e: number } }
}
declare module "x.css" {
  interface /*@X:d*/Styles { /*@!*/b: string }
}
