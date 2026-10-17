export {};
type IdentityPart<P extends string> = P extends `${string}#${infer Id}@${string}` ? Id : never;

type WholeDistinct<A extends string, B extends string> = `pre:${A}:tail` extends `pre:${B}:tail` ? true : false;
function wholeDistinct<K extends string, L extends string>() {
    const Outer: { [P in K]: (outer: IdentityPart<P>) => unknown } = class {
        static #outer = (outer) => {
            const Inner: { [Q in L]: (inner: [typeof outer, IdentityPart<Q>, WholeDistinct<typeof outer, IdentityPart<Q>>]) => unknown } = class {
                static #inner = (inner) => /*#wholeDistinct*/inner;
            };
        };
    };
}

type AdjacentPartition<A extends string, B extends string> = `${A}${B}` extends `${B}${A}` ? true : false;
function adjacentPartition<K extends string, L extends string>() {
    const Outer: { [P in K]: (outer: IdentityPart<P>) => unknown } = class {
        static #outer = (outer) => {
            const Inner: { [Q in L]: (inner: [typeof outer, IdentityPart<Q>, AdjacentPartition<typeof outer, IdentityPart<Q>>]) => unknown } = class {
                static #inner = (inner) => /*#adjacentPartition*/inner;
            };
        };
    };
}

type SuffixDecides<A extends string> = `${A}x` extends "1y" ? true : false;
function suffixDecides<K extends string, L extends string>() {
    const Outer: { [P in K]: (outer: IdentityPart<P>) => unknown } = class {
        static #outer = (outer) => {
            const Inner: { [Q in L]: (inner: [typeof outer, IdentityPart<Q>, SuffixDecides<typeof outer>]) => unknown } = class {
                static #inner = (inner) => /*#suffixDecides*/inner;
            };
        };
    };
}
