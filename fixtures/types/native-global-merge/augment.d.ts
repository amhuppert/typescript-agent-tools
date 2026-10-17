interface RelativeIndexable<T> {
    at(index: number): T | undefined;
}
interface Array<T> extends RelativeIndexable<T> {
    find(predicate: (value: T) => boolean): T | undefined;
}
interface ReadonlyArray<T> extends RelativeIndexable<T> {}
interface Box<T> {
    second: T;
    secondNested: { value: T };
    callback: (value: T) => void;
    self(): this;
}
declare function pick(value: number): "second-number";
declare function pick(value: string): "second-string";
declare enum Color { Green = 2 }
