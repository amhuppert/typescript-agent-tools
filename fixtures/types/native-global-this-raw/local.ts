export {};
function globalThis(value: string): "called";
function globalThis(value: unknown): "called" { return "called"; }
const bad = globalThis(123/*#badCall*/);
const good = globalThis("ok"/*#goodCall*/);
const /*#declaration*/same = globalThis;
