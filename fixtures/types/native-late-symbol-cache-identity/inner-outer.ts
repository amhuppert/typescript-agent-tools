export {};
declare const key: "value";
class Outer {
    get [key]() { return 61 as const; }
    set [key](outerInput) {
        class Inner {
            get [key]() { return 62 as const; }
            set [key](innerInput) {
                const /*#chosen*/chosen = (void innerInput, outerInput);
                let healthy: number;
            }
        }
    }
}
