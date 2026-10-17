type R<T> = T extends {x:unknown} ? R<T["x"]> : number;
interface Host<X extends R<X>> { value: X; }
