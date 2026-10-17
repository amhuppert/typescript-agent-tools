declare const box: Box<string>;
export const /*#array*/xs = [1, 2];
export const /*#at*/first = xs.at(0);
export const /*#merged*/second = box.second;
export const /*#own*/own = box.first;
export const /*#firstNested*/firstNested = box.firstNested.value;
export const /*#secondNested*/secondNested = box.secondNested.value;
export const /*#callback*/callback = box.callback;
export const /*#self*/self = box.self();
export const found = xs.find(/*#element*/element => element === 1);
export const /*#picked*/picked = pick(1);
export const /*#green*/green = Color.Green;

const key = "x" as const;
declare const symbolKey: unique symbol;
class Accessors {
    get [key]() { return (/*#literalGetter*/value) => value; }
    set [key](value: (input: number) => number) {}
    get [symbolKey]() { return (/*#symbolGetter*/value) => value; }
    set [symbolKey](value: (input: string) => string) {}
    get plain() { return (/*#plainGetter*/value) => value; }
    set plain(value: (input: boolean) => boolean) {}
}
export const accessors = new Accessors();
