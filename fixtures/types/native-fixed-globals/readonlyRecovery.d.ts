interface Array<T> { length: number; [index: number]: T; }
type ReadonlyArray<T> = MissingReadonly;
type ThisType<T> = MissingThis;
