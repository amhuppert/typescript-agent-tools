type R<T> = T extends string ? R<T> : number;
interface Host<X extends R<X>> { value: X; }
