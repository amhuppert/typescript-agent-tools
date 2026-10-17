export {};
declare function m(cb: (x: string) => number): 'first';
declare function m(cb: (x: any) => string): 'second';
const /*#result*/result = m(/*#parameter*/x => /*#body*/x.toUpperCase());
interface Shout { /*#origin*/shout(): string }
declare function q(cb: (x: Shout) => number): 'first';
declare function q(cb: (x: any) => string): 'second';
const /*#memberResult*/qr = q(/*#memberParameter*/x => x./*#member*/shout());
