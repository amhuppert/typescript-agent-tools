type R<T> = T extends string ? R<{[P in keyof T]: T[P]}> : number;
interface Host<X extends R<X>> { value: X; }
