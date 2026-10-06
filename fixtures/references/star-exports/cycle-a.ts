// An `export *` cycle terminates; each module still exports its own names.
export * from "./cycle-b";
export const /*@fromA:de*/fromA = 1;
