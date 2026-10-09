// A class augmentation gives this interface a value, which this module's own
// use and its importers see, although it is exported type-only.
interface /*@Shape:d*/Shape { /*@!*/s: number }
export type { /*@Shape:e*/Shape };
export const /*@made:de*/made = new /*@Shape:u*/Shape();
