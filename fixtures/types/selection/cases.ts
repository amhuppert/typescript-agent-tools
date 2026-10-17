export {};
class /*#class*/Widget { n = 1; static label = 'widget'; }
const constructor = /*#classUse*/Widget;
declare const instance: Widget;
instance./*#access*/n;
instance[/*#key*/'n'];
type Box<T> = { value: T; next?: Box<T> };
type /*#alias*/StringBox = Box<string>;
declare const boxed: /*#type*/Box<string>;
const callback: (x: string) => number = /*#contextParameter*/x => x.length;
const rejected = { f: (/*#satisfiesParameter*/x) => String(x) } satisfies { f: (x: number) => number };
