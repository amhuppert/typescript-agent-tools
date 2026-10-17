type R<T> = T extends string ? R<T & {x:1}> : number;
interface Host<X extends R<X>> { value: X; }
