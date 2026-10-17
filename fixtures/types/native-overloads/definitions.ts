export declare function choose(value: string): "broad";
export declare function choose(value: "exact"): "exact";
export declare function choose(value: number): "number";
export declare function choose(value: true): "yes";
export declare function ordered(value: number): "number";
export declare function ordered(value: any): "any";
export declare function arity(value: number): "one";
export declare function arity(value: number, extra: number): "two";
export declare function omitted(value: number): "number";
export declare function omitted(value: void): "void";
export declare function thisOrder(this: number): "number";
export declare function thisOrder(this: void): "void";
export declare function tied(value: number): "first";
export declare function tied(value: number): "second";
export interface Callable {
    (value: string): "broad-call";
    (value: "exact"): "exact-call";
}
export declare const callable: Callable;
