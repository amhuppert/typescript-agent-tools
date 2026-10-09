// A generic interface exported through a type-only specifier. Two files
// augment it; every declaration is one symbol sharing one type parameter.
interface /*@Commands:d*/Commands</*@T:d*/T = any> { /*@!*/base: /*@T:u*/T }
export type { /*@Commands:e*/Commands };
export declare const /*@local:de*/local: /*@Commands:u*/Commands;
