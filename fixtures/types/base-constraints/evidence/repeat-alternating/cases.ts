type A<T> = T extends string ? B<{[P in keyof T]:T[P]}> : number; type B<T> = T extends string ? A<{[P in keyof T]:T[P]}> : number; type R<T> = A<T>;
interface Host<X extends R<X>> { value: X; }
