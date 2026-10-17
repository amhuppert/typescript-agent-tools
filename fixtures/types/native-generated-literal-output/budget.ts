export {};
type IdentityPart<P extends string> = P extends `${string}#${infer Id}@${string}` ? Id : never;
function budget<K extends string>() {
    const C: { [P in K]: (arg: `__PADDING__${IdentityPart<P>}`) => unknown } =
    class {
        static #field = (arg) => /*#budget*/arg;
    };
}
