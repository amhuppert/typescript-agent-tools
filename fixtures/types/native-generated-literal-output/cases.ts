export {};
type IdentityPart<P extends string> = P extends `${string}#${infer Id}@${string}` ? Id : never;
type RawPrefix<P extends string> = P extends `${infer Prefix}#${string}` ? Prefix : never;

function rawPrefix<K extends string>() {
    const C: { [P in K]: (arg: RawPrefix<P>) => unknown } = class {
        static #field = (arg) => /*#rawPrefix*/arg;
    };
}
function repeated<K extends string>() {
    const C: { [P in K]: (arg: `${IdentityPart<P>}:${IdentityPart<P>}`) => unknown } = class {
        static #field = (arg) => /*#repeated*/arg;
    };
}
function sharedTuple<K extends string>() {
    const C: { [P in K]: (arg: [IdentityPart<P>, IdentityPart<P>]) => unknown } = class {
        static #field = (arg) => /*#sharedTuple*/arg;
    };
}
function sharedGraph<K extends string>() {
    const C: { [P in K]: (arg: { direct: IdentityPart<P>; prefixed: `pre:${IdentityPart<P>}`; again: IdentityPart<P> }) => unknown } = class {
        static #field = (arg) => /*#sharedGraph*/arg;
    };
}
function distinctTuple<K extends string, L extends string>() {
    const Outer: { [P in K]: (outer: IdentityPart<P>) => unknown } = class {
        static #outer = (outer) => {
            const Inner: { [Q in L]: (inner: [typeof outer, IdentityPart<Q>, typeof outer]) => unknown } = class {
                static #inner = (inner) => /*#distinctTuple*/inner;
            };
        };
    };
}
declare const replacement: "�";
const replacementResult = /*#replacement*/replacement;
declare const lookalike: "string(symbol(s0))";
const lookalikeResult = /*#lookalike*/lookalike;
