export {};
function publicStatic<K extends string>() {
    const C: { [P in K]: (arg: P) => P } = class {
        static field = (arg) => /*#publicStatic*/arg;
    };
}
function privateStatic<K extends string>() {
    const C: { [P in K]: (arg: P) => P } = class {
        static #field = (arg) => /*#privateStatic*/arg;
    };
}
function privateInstance<K extends string>() {
    const C: { [P in K]: (arg: P) => P } = class {
        #field = (arg) => /*#privateInstance*/arg;
    };
}
type IdentityPart<P extends string> = P extends `${string}#${infer Id}@${string}` ? Id : never;
type IdentityHead<P extends string> = IdentityPart<P> extends `${infer First}${infer Rest}` ? First : never;
function privateIdentity<K extends string>() {
    const C: { [P in K]: (arg: IdentityPart<P>) => unknown } = class {
        static #field = (arg) => /*#privateIdentity*/arg;
    };
}
function privateIdentityHead<K extends string>() {
    const C: { [P in K]: (arg: IdentityHead<P>) => unknown } = class {
        static #field = (arg) => /*#privateIdentityHead*/arg;
    };
}
function privateConcat<K extends string>() {
    const C: { [P in K]: (arg: `prefix:${P}:suffix`) => unknown } = class {
        static #field = (arg) => /*#privateConcat*/arg;
    };
}
function privateUpper<K extends string>() {
    const C: { [P in K]: (arg: Uppercase<P>) => unknown } = class {
        static #field = (arg) => /*#privateUpper*/arg;
    };
}
function privateLower<K extends string>() {
    const C: { [P in K]: (arg: Lowercase<P>) => unknown } = class {
        static #Field = (arg) => /*#privateLower*/arg;
    };
}
function privateCapitalize<K extends string>() {
    const C: { [P in K]: (arg: Capitalize<P>) => unknown } = class {
        static #field = (arg) => /*#privateCapitalize*/arg;
    };
}
function privateUncapitalize<K extends string>() {
    const C: { [P in K]: (arg: Uncapitalize<P>) => unknown } = class {
        static #Field = (arg) => /*#privateUncapitalize*/arg;
    };
}
class KeyOwner { #secret = 1; public = 1; }
declare const keys: keyof KeyOwner;
const keyResult = /*#keyofControl*/keys;
declare const mapped: { [P in keyof KeyOwner]: P };
const mappedResult = mapped./*#mappedControl*/public;
