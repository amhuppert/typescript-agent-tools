export declare let selected: string | number;
declare const source: { item: string | number; latent: MissingSibling };
declare const shorthandSource: { selected: string | number };
declare const numeric: { 1: string | number };
declare const exact: { "a\u0062": string | number };
declare const defaultSource: { item: string | number | undefined };
declare const arraySource: Array<string | number>;
declare const defaultArray: Array<string | number | undefined>;
declare const key: "item";
declare const fallback: number;

export function ordinaryProperty(): void {
    ({ item: selected } = source);
    /*#ownProperty*/selected;
}
export function shorthand(): void {
    ({ selected } = shorthandSource);
    /*#shorthand*/selected;
}
export function numericName(): void {
    ({ 0x1: selected } = numeric);
    /*#numericName*/selected;
}
export function exactName(): void {
    ({ "ab": selected } = exact);
    /*#exactName*/selected;
}
export function computedName(): void {
    ({ [key]: selected } = source);
    /*#computedName*/selected;
}
export function propertyDefault(): void {
    ({ item: selected = fallback } = defaultSource);
    /*#propertyDefault*/selected;
}
export function arrayElement(): void {
    ([selected] = arraySource);
    /*#arrayElement*/selected;
}
export function arrayDefault(): void {
    ([selected = fallback] = defaultArray);
    /*#arrayDefault*/selected;
}
export function omittedSlot(): void {
    ([, selected = fallback] = defaultArray);
    /*#omittedSlot*/selected;
}
export function arraySpread(): void {
    ([...selected] = arraySource);
    /*#arraySpread*/selected;
}
export function forOf(): void {
    for (selected of arraySource) {
        /*#forOf*/selected;
    }
}
