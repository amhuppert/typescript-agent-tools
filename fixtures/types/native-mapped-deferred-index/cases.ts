export {};
type Deferred<T> = { [P in keyof T as T extends string ? keyof Deferred<T> : keyof Deferred<T>]: P };
declare function choose<T>(key: keyof Deferred<T>): 1;
declare function choose<T>(key: unknown): 2;
function observe<T>(text: string) {
    const /*#literal*/literal = choose<T>('x');
    const /*#text*/broad = choose<T>(text);
    return [literal, broad];
}
type Fixed<T> = { [P in keyof T as "fixed"]: P };
declare function fixed<T>(key: keyof Fixed<T>): 1;
declare function fixed<T>(key: unknown): 2;
function fixedControls<T>() {
    const /*#positive*/positive = fixed<T>('fixed');
    const /*#negative*/negative = fixed<T>('other');
    return [positive, negative];
}
