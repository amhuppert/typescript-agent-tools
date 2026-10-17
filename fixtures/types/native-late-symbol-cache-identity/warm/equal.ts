export {};
declare function warm(value: 0): 0;
declare function warm(value: unknown): 0;
declare const key: "value";
class Outer {
    get [key]() { return 61 as const; }
    set [key](outerInput) {
        class Inner {
            get [key]() { return 61 as const; }
            set [key](innerInput) {
                const /*#chosen*/chosen = (warm(outerInput), innerInput);
                let healthy: number;
            }
        }
    }
}
