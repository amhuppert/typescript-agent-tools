export {};
declare const /*#declaration*/fn: any;
(/*#paren*/fn)();
(/*#nonNull*/fn!)();
(<typeof fn>/*#angle*/fn)();
(/*#as*/fn as typeof fn)();
(/*#satisfies*/fn satisfies typeof fn)();
/*#optional*/fn?.();
new /*#constructor*/fn;
/*#tag*/fn`tag`;
declare function consume(value: unknown): void;
consume(/*#argument*/fn);
const copy = /*#read*/fn;
/*#callProperty*/fn.call(null);
@/*#implicitDecorator*/fn class Implicit {}
@/*#explicitDecorator*/fn() class Explicit {}
const ns = { fn };
/*#namespaceReceiver*/ns./*#namespaceMember*/fn();
