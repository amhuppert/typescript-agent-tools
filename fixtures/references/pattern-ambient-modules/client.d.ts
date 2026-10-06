// Both patterns have an empty prefix, so "./x.module.css" takes the first
// declared, as TypeScript's `findBestPatternMatch` keeps the earliest.
declare module "*.module.css" { export const /*@onlyModule:de*/onlyModule: number; }
declare module "*.css" { export const /*@onlyPlain:de*/onlyPlain: number; }
