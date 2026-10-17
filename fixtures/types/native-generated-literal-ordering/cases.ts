export {};
type IdentityPart<P extends string> = P extends `${string}#${infer Id}@${string}` ? Id : never;
type RawPrefix<P extends string> = P extends `${infer Prefix}#${string}` ? Prefix : never;

function sharedSuffix<K extends string>() {
    const C: { [P in K]: (arg: [IdentityPart<P>, `${IdentityPart<P>}:b` | `${IdentityPart<P>}:a`]) => unknown } = class {
        static #field = (arg) => /*#sharedSuffix*/arg;
    };
}

function sharedSuffixReverse<K extends string>() {
    const C: { [P in K]: (arg: [IdentityPart<P>, `${IdentityPart<P>}:a` | `${IdentityPart<P>}:b`]) => unknown } = class {
        static #field = (arg) => /*#sharedSuffixReverse*/arg;
    };
}

function sharedPrefix<K extends string>() {
    const C: { [P in K]: (arg: [IdentityPart<P>, `${IdentityPart<P>}:tail` | IdentityPart<P>]) => unknown } = class {
        static #field = (arg) => /*#sharedPrefix*/arg;
    };
}

function repeatedAtom<K extends string>() {
    const C: { [P in K]: (arg: [IdentityPart<P>, `${IdentityPart<P>}${IdentityPart<P>}:b` | `${IdentityPart<P>}${IdentityPart<P>}:a`]) => unknown } = class {
        static #field = (arg) => /*#repeatedAtom*/arg;
    };
}

function fixedPrefix<K extends string>() {
    const C: { [P in K]: (arg: [IdentityPart<P>, `z${IdentityPart<P>}` | `a${IdentityPart<P>}`]) => unknown } = class {
        static #field = (arg) => /*#fixedPrefix*/arg;
    };
}

function empty<K extends string>() {
    const C: { [P in K]: (arg: [IdentityPart<P>, IdentityPart<P> | ""]) => unknown } = class {
        static #field = (arg) => /*#empty*/arg;
    };
}

function emptyReverse<K extends string>() {
    const C: { [P in K]: (arg: [IdentityPart<P>, "" | IdentityPart<P>]) => unknown } = class {
        static #field = (arg) => /*#emptyReverse*/arg;
    };
}

function nul<K extends string>() {
    const C: { [P in K]: (arg: [IdentityPart<P>, IdentityPart<P> | "\0"]) => unknown } = class {
        static #field = (arg) => /*#nul*/arg;
    };
}

function nulReverse<K extends string>() {
    const C: { [P in K]: (arg: [IdentityPart<P>, "\0" | IdentityPart<P>]) => unknown } = class {
        static #field = (arg) => /*#nulReverse*/arg;
    };
}

function zero<K extends string>() {
    const C: { [P in K]: (arg: [IdentityPart<P>, IdentityPart<P> | "0"]) => unknown } = class {
        static #field = (arg) => /*#zero*/arg;
    };
}

function zeroReverse<K extends string>() {
    const C: { [P in K]: (arg: [IdentityPart<P>, "0" | IdentityPart<P>]) => unknown } = class {
        static #field = (arg) => /*#zeroReverse*/arg;
    };
}

function colon<K extends string>() {
    const C: { [P in K]: (arg: [IdentityPart<P>, ":" | IdentityPart<P>]) => unknown } = class {
        static #field = (arg) => /*#colon*/arg;
    };
}

function colonReverse<K extends string>() {
    const C: { [P in K]: (arg: [IdentityPart<P>, IdentityPart<P> | ":"]) => unknown } = class {
        static #field = (arg) => /*#colonReverse*/arg;
    };
}

function rawFE<K extends string>() {
    const C: { [P in K]: (arg: [IdentityPart<P>, RawPrefix<P> | "�"]) => unknown } = class {
        static #field = (arg) => /*#rawFE*/arg;
    };
}

function uniqueCycle<K extends string>() {
    const C: { [P in K]: (arg: [IdentityPart<P>, `${RawPrefix<P>}@a@${IdentityPart<P>}` | `${RawPrefix<P>}@a0@${IdentityPart<P>}` | `${RawPrefix<P>}@a9${IdentityPart<P>}`]) => unknown } = class {
        static #field = (arg) => /*#uniqueCycle*/arg;
    };
}

function uniqueCycleReverse<K extends string>() {
    const C: { [P in K]: (arg: [IdentityPart<P>, `${RawPrefix<P>}@a9${IdentityPart<P>}` | `${RawPrefix<P>}@a0@${IdentityPart<P>}` | `${RawPrefix<P>}@a@${IdentityPart<P>}`]) => unknown } = class {
        static #field = (arg) => /*#uniqueCycleReverse*/arg;
    };
}

function templateSpans<K extends string>() {
    const C: { [P in K]: (arg: [IdentityPart<P>, ((`pre:${IdentityPart<P>}:b${number}` | `pre:${IdentityPart<P>}:a${number}`) extends string ? "ordered" : "bad")]) => unknown } = class {
        static #field = (arg) => /*#templateSpans*/arg;
    };
}

function digit1<K extends string>() {
    const C: { [P in K]: (arg: [IdentityPart<P>, `${IdentityPart<P>}x` | "1y"]) => unknown } = class {
        static #field = (arg) => /*#digit1*/arg;
    };
}

function digit2<K extends string>() {
    const C: { [P in K]: (arg: [IdentityPart<P>, `${IdentityPart<P>}x` | "2y"]) => unknown } = class {
        static #field = (arg) => /*#digit2*/arg;
    };
}

function digit3<K extends string>() {
    const C: { [P in K]: (arg: [IdentityPart<P>, `${IdentityPart<P>}x` | "3y"]) => unknown } = class {
        static #field = (arg) => /*#digit3*/arg;
    };
}

function digit4<K extends string>() {
    const C: { [P in K]: (arg: [IdentityPart<P>, `${IdentityPart<P>}x` | "4y"]) => unknown } = class {
        static #field = (arg) => /*#digit4*/arg;
    };
}

function digit5<K extends string>() {
    const C: { [P in K]: (arg: [IdentityPart<P>, `${IdentityPart<P>}x` | "5y"]) => unknown } = class {
        static #field = (arg) => /*#digit5*/arg;
    };
}

function digit6<K extends string>() {
    const C: { [P in K]: (arg: [IdentityPart<P>, `${IdentityPart<P>}x` | "6y"]) => unknown } = class {
        static #field = (arg) => /*#digit6*/arg;
    };
}

function digit7<K extends string>() {
    const C: { [P in K]: (arg: [IdentityPart<P>, `${IdentityPart<P>}x` | "7y"]) => unknown } = class {
        static #field = (arg) => /*#digit7*/arg;
    };
}

function digit8<K extends string>() {
    const C: { [P in K]: (arg: [IdentityPart<P>, `${IdentityPart<P>}x` | "8y"]) => unknown } = class {
        static #field = (arg) => /*#digit8*/arg;
    };
}

function digit9<K extends string>() {
    const C: { [P in K]: (arg: [IdentityPart<P>, `${IdentityPart<P>}x` | "9y"]) => unknown } = class {
        static #field = (arg) => /*#digit9*/arg;
    };
}

function unequalAtoms<K extends string, L extends string>() {
    const Outer: { [P in K]: (outer: IdentityPart<P>) => unknown } = class {
        static #outer = (outer) => {
            const Inner: { [Q in L]: (inner: [typeof outer, IdentityPart<Q>, typeof outer | IdentityPart<Q>]) => unknown } = class {
                static #inner = (inner) => /*#unequalAtoms*/inner;
            };
        };
    };
}

function adjacentPartitions<K extends string, L extends string>() {
    const Outer: { [P in K]: (outer: IdentityPart<P>) => unknown } = class {
        static #outer = (outer) => {
            const Inner: { [Q in L]: (inner: [typeof outer, IdentityPart<Q>, `${typeof outer}${IdentityPart<Q>}x` | `${IdentityPart<Q>}${typeof outer}y`]) => unknown } = class {
                static #inner = (inner) => /*#adjacentPartitions*/inner;
            };
        };
    };
}
