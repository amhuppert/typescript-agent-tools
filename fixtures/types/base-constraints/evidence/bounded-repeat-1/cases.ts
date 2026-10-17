interface Array<T> { length: number; }
type R<T, N extends number, C extends unknown[] = []> = C["length"] extends N ? "done" : T extends string ? R<{[P in keyof T]:T[P]}, N, [...C,unknown]> : number;
interface Host<T> { value: R<T,1>; }
