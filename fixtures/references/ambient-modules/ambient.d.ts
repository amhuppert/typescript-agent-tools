// Ambient modules satisfy imports without files on disk.
declare module "virtual" {
  export const /*@virtualConst*/virtualConst: number;
  export interface /*@VirtualShape*/VirtualShape { /*@!*/size: number }
}

declare module "pattern/*" {
  export function /*@patterned*/patterned(): void;
}
