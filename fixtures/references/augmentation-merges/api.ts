// An augmentation's namespace merges with this one, and its interface with
// what the exported import alias names.
export namespace /*@Other:de*/Other { export interface /*@X:de*/X { /*@!*/a: number } }
export namespace /*@N:de*/N { export import /*@X:ei*/X = /*@Other:u*/Other./*@X:u*/X; }
