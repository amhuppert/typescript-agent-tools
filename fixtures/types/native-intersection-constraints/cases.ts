declare const ordinary: { p: string } & { q: number };
const ordinaryValue = /*#ordinary*/ordinary.p;

function changed<T extends { p: string } | { p: number }, U extends { q: boolean }>(value: T & U) {
    const result = /*#changed*/value.p;
}

function absent<T extends { p: string } | { p: number }, U>(value: T & U) {
    const result = /*#absent*/value.p;
}

function nested<U extends { q: string } | { q: number }, T extends U & { p: boolean }>(value: T) {
    const result = /*#nested*/value.p;
}

function nullable<T extends { p: string } | undefined>(value: T & { q: number }) {
    const result = /*#nullable*/value.p;
}

function unchanged<T extends { p: string }, U extends { q: number }>(value: T & U) {
    const result = /*#unchanged*/value.p;
}

function invalid<T extends { p: string } | { p: number }, U extends MissingConstraint>(value: T & U) {
    const result = /*#invalid*/value.p;
}
