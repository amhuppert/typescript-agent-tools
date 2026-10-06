// Ambient modules satisfy imports without files on disk.
declare module "virtual" {
  export const /*@virtualConst:de*/virtualConst: number;
  export interface /*@VirtualShape:de*/VirtualShape { /*@!*/size: number }
}

declare module "pattern/*" {
  export function /*@patterned:de*/patterned(): void;
}
