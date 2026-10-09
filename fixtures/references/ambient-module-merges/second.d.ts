declare module "aliased" {
  interface /*@X:d*/X { /*@!*/b: number }
}
declare module "short" {
  export const /*@listed:de*/listed: number;
}
declare module "*.view" {
  export interface /*@B:de*/B extends /*@A:u*/A {}
}
