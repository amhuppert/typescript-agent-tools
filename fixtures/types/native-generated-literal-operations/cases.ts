export {};
type Tail<P extends string> = P extends `${string}@${infer T}` ? T : never;
type Identity<P extends string> = P extends `${string}#${infer Id}@${string}` ? Id : never;
type Head<P extends string> = P extends `${infer First}${infer Rest}` ? First : never;
type PrefixKind<P extends string> = P extends `�${string}` ? "replacement" : "raw";

type OperationConcatTail<P extends string> = Tail<`prefix:${P}:suffix`>;
function concatTail<K extends string>() {
    const C: { [P in K]: (arg: OperationConcatTail<P>) => unknown } = class {
        static #field = (arg) => /*#concatTail*/arg;
    };
}

type OperationConcatRawPrefix<P extends string> = `prefix:${P}:suffix` extends `prefix:�${string}` ? "replacement" : "raw";
function concatRawPrefix<K extends string>() {
    const C: { [P in K]: (arg: OperationConcatRawPrefix<P>) => unknown } = class {
        static #field = (arg) => /*#concatRawPrefix*/arg;
    };
}

type OperationRawHead<P extends string> = Head<P> extends "�" ? "replacement" : "raw";
function rawHead<K extends string>() {
    const C: { [P in K]: (arg: OperationRawHead<P>) => unknown } = class {
        static #field = (arg) => /*#rawHead*/arg;
    };
}

type OperationReplacementPrefix<P extends string> = PrefixKind<`�#${Identity<P>}@#field`>;
function replacementPrefix<K extends string>() {
    const C: { [P in K]: (arg: OperationReplacementPrefix<P>) => unknown } = class {
        static #field = (arg) => /*#replacementPrefix*/arg;
    };
}

type OperationUpper<P extends string> = `${PrefixKind<Uppercase<P>>}:${Tail<Uppercase<P>>}`;
function upper<K extends string>() {
    const C: { [P in K]: (arg: OperationUpper<P>) => unknown } = class {
        static #field = (arg) => /*#upper*/arg;
    };
}

type OperationLower<P extends string> = `${PrefixKind<Lowercase<P>>}:${Tail<Lowercase<P>>}`;
function lower<K extends string>() {
    const C: { [P in K]: (arg: OperationLower<P>) => unknown } = class {
        static #Field = (arg) => /*#lower*/arg;
    };
}

type OperationCapitalize<P extends string> = `${PrefixKind<Capitalize<P>>}:${Tail<Capitalize<P>>}`;
function capitalize<K extends string>() {
    const C: { [P in K]: (arg: OperationCapitalize<P>) => unknown } = class {
        static #field = (arg) => /*#capitalize*/arg;
    };
}

type OperationUncapitalize<P extends string> = `${PrefixKind<Uncapitalize<P>>}:${Tail<Uncapitalize<P>>}`;
function uncapitalize<K extends string>() {
    const C: { [P in K]: (arg: OperationUncapitalize<P>) => unknown } = class {
        static #Field = (arg) => /*#uncapitalize*/arg;
    };
}

type OperationCapitalizeRawRemainder<P extends string> = Capitalize<`x${P}`> extends `X�${string}` ? "replacement" : "raw";
function capitalizeRawRemainder<K extends string>() {
    const C: { [P in K]: (arg: OperationCapitalizeRawRemainder<P>) => unknown } = class {
        static #field = (arg) => /*#capitalizeRawRemainder*/arg;
    };
}

type OperationUncapitalizeRawRemainder<P extends string> = Uncapitalize<`X${P}`> extends `x�${string}` ? "replacement" : "raw";
function uncapitalizeRawRemainder<K extends string>() {
    const C: { [P in K]: (arg: OperationUncapitalizeRawRemainder<P>) => unknown } = class {
        static #field = (arg) => /*#uncapitalizeRawRemainder*/arg;
    };
}

type OperationUpperExpansion<P extends string> = Uppercase<`ß${Identity<P>}ﬃ`> extends `SS${number}FFI` ? "expanded" : "wrong";
function upperExpansion<K extends string>() {
    const C: { [P in K]: (arg: OperationUpperExpansion<P>) => unknown } = class {
        static #field = (arg) => /*#upperExpansion*/arg;
    };
}

type OperationCapitalizeExpansion<P extends string> = Capitalize<`ß${Identity<P>}tail`> extends `SS${number}tail` ? "expanded" : "wrong";
function capitalizeExpansion<K extends string>() {
    const C: { [P in K]: (arg: OperationCapitalizeExpansion<P>) => unknown } = class {
        static #field = (arg) => /*#capitalizeExpansion*/arg;
    };
}

type OperationUncapitalizeExpansion<P extends string> = Uncapitalize<`İ${Identity<P>}TAIL`> extends `i̇${number}TAIL` ? "expanded" : "wrong";
function uncapitalizeExpansion<K extends string>() {
    const C: { [P in K]: (arg: OperationUncapitalizeExpansion<P>) => unknown } = class {
        static #field = (arg) => /*#uncapitalizeExpansion*/arg;
    };
}

type OperationSigmaBeforeIdentity<P extends string> = Lowercase<`AΣ${Identity<P>}ΣA`> extends `aς${number}σa` ? "sigma-boundary" : "wrong";
function sigmaBeforeIdentity<K extends string>() {
    const C: { [P in K]: (arg: OperationSigmaBeforeIdentity<P>) => unknown } = class {
        static #field = (arg) => /*#sigmaBeforeIdentity*/arg;
    };
}

type OperationSigmaMarksBesideIdentity<P extends string> = Lowercase<`AΣ́${Identity<P>}Σ́A`> extends `aς́${number}σ́a` ? "sigma-boundary" : "wrong";
function sigmaMarksBesideIdentity<K extends string>() {
    const C: { [P in K]: (arg: OperationSigmaMarksBesideIdentity<P>) => unknown } = class {
        static #field = (arg) => /*#sigmaMarksBesideIdentity*/arg;
    };
}

type OperationSigmaAfterIdentity<P extends string> = Lowercase<`${Identity<P>}Σ`> extends `${number}σ` ? "uncased-boundary" : "wrong";
function sigmaAfterIdentity<K extends string>() {
    const C: { [P in K]: (arg: OperationSigmaAfterIdentity<P>) => unknown } = class {
        static #field = (arg) => /*#sigmaAfterIdentity*/arg;
    };
}

type OperationJoinedSurrogate<P extends string> = Tail<`${P}:\uD83D${""}\uDE00`>;
function joinedSurrogate<K extends string>() {
    const C: { [P in K]: (arg: OperationJoinedSurrogate<P>) => unknown } = class {
        static #field = (arg) => /*#joinedSurrogate*/arg;
    };
}

type OperationJoinedNestedSurrogate<P extends string> = Tail<`${P}:\uD83D${`${""}`}\uDE00`>;
function joinedNestedSurrogate<K extends string>() {
    const C: { [P in K]: (arg: OperationJoinedNestedSurrogate<P>) => unknown } = class {
        static #field = (arg) => /*#joinedNestedSurrogate*/arg;
    };
}

type OperationLoneHigh<P extends string> = Tail<`${P}:\uD800${""}`> extends `#field:\uD800${string}` ? Tail<`${P}:\uD800${""}`> extends `#field:�${string}` ? "replacement" : "high-preserved" : "wrong";
function loneHigh<K extends string>() {
    const C: { [P in K]: (arg: OperationLoneHigh<P>) => unknown } = class {
        static #field = (arg) => /*#loneHigh*/arg;
    };
}

type OperationLoneLow<P extends string> = Tail<`${P}:\uDE00${""}`> extends `#field:\uDE00${string}` ? Tail<`${P}:\uDE00${""}`> extends `#field:�${string}` ? "replacement" : "low-preserved" : "wrong";
function loneLow<K extends string>() {
    const C: { [P in K]: (arg: OperationLoneLow<P>) => unknown } = class {
        static #field = (arg) => /*#loneLow*/arg;
    };
}

type OperationReversedSurrogates<P extends string> = Tail<`${P}:\uDE00${""}\uD83D`> extends `#field:\uDE00\uD83D${string}` ? "order-preserved" : "wrong";
function reversedSurrogates<K extends string>() {
    const C: { [P in K]: (arg: OperationReversedSurrogates<P>) => unknown } = class {
        static #field = (arg) => /*#reversedSurrogates*/arg;
    };
}

type OperationMappedLoneSurrogate<P extends string> = Tail<Uppercase<`${P}:\uD800${""}x`>> extends `#FIELD:\uD800${string}` ? "sentinel-preserved" : "wrong";
function mappedLoneSurrogate<K extends string>() {
    const C: { [P in K]: (arg: OperationMappedLoneSurrogate<P>) => unknown } = class {
        static #field = (arg) => /*#mappedLoneSurrogate*/arg;
    };
}

type OperationSurrogatesSeparatedByIdentity<P extends string> = `\uD83D${Identity<P>}\uDE00` extends `\uD83D${number}\uDE00` ? "identity-separates" : "wrong";
function surrogatesSeparatedByIdentity<K extends string>() {
    const C: { [P in K]: (arg: OperationSurrogatesSeparatedByIdentity<P>) => unknown } = class {
        static #field = (arg) => /*#surrogatesSeparatedByIdentity*/arg;
    };
}

type OperationTemplateSpanCapture<P extends string> = `${P}:${string}:tail` extends `${string}:tail` ? `${P}:x${string}` extends `${P}:${string}` ? "captured" : "wrong" : "wrong";
function templateSpanCapture<K extends string>() {
    const C: { [P in K]: (arg: OperationTemplateSpanCapture<P>) => unknown } = class {
        static #field = (arg) => /*#templateSpanCapture*/arg;
    };
}

type OperationTemplateSpanMapping<P extends string> = Uppercase<`prefix:${P}:${string}:tail`> extends `PREFIX:${string}:TAIL` ? "mapped" : "wrong";
function templateSpanMapping<K extends string>() {
    const C: { [P in K]: (arg: OperationTemplateSpanMapping<P>) => unknown } = class {
        static #field = (arg) => /*#templateSpanMapping*/arg;
    };
}

type OperationSameIdentity<P extends string> = Identity<P> extends Identity<P> ? "same" : "wrong";
function sameIdentity<K extends string>() {
    const C: { [P in K]: (arg: OperationSameIdentity<P>) => unknown } = class {
        static #field = (arg) => /*#sameIdentity*/arg;
    };
}

type OperationIdentityNondigit<P extends string> = Identity<P> extends "not-digits" ? "wrong" : "nondigit";
function identityNondigit<K extends string>() {
    const C: { [P in K]: (arg: OperationIdentityNondigit<P>) => unknown } = class {
        static #field = (arg) => /*#identityNondigit*/arg;
    };
}

type OperationIdentityConcreteDigits<P extends string> = Identity<P> extends "1" ? "same" : "different";
function identityConcreteDigits<K extends string>() {
    const C: { [P in K]: (arg: OperationIdentityConcreteDigits<P>) => unknown } = class {
        static #field = (arg) => /*#identityConcreteDigits*/arg;
    };
}

