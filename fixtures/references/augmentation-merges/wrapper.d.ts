// A module directly inside a script's ambient module augments its package.
declare module "wrapper" {
  module "kit" {
    interface /*@Kit:d*/Kit { /*@!*/extra: number }
  }
}
