export {};
interface Box<T> { /*#generic*/value: T; extra: boolean; }
declare const strings: Box<string>;
declare const numbers: Box<number>;
strings./*#stringUse*/value;
numbers./*#numberUse*/value;
declare const partial: Partial<Box<string>>;
partial./*#partial*/value;
declare const readonly: Readonly<Box<string>>;
readonly./*#readonly*/value;
declare const picked: Pick<Box<string>, 'value'>;
picked./*#picked*/value;
declare const omitted: Omit<Box<string>, 'extra'>;
omitted./*#omitted*/value;
declare const remapped: { [K in keyof Box<string> as `new_${K}`]: Box<string>[K] };
remapped./*#remapped*/new_value;
const ordinarySpread = { ...strings };
ordinarySpread./*#ordinarySpread*/value;
const overlapSpread = { ...partial, ...numbers };
overlapSpread./*#overlapSpread*/value;
const getOnly = { get /*#getOnly*/value(): string { return 's'; } };
const getSpread = { ...getOnly };
getSpread./*#getSpread*/value;
const setOnly = { set /*#setOnly*/value(v: string) {} };
const setSpread = { ...setOnly };
setSpread./*#setSpread*/value;
const readonlySource = { value: 'r' } as const;
const readonlySpread = { ...readonlySource };
readonlySpread./*#readonlySpread*/value;
interface Host { /*#host*/run(): string; }
const annotated: Host = { /*#annotated*/run() { return 'a'; } };
const satisfied = { /*#satisfied*/run() { return 's'; } } satisfies Host;
const unrelated = { /*#unrelated*/run() { return 'u'; } };
annotated./*#annotatedUse*/run();
satisfied./*#satisfiedUse*/run();
unrelated./*#unrelatedUse*/run();
class Base { /*#base*/act(): void {} }
class Derived extends Base { override /*#override*/act(): void {} }
interface Action { /*#interface*/act(): void; }
class Implementation implements Action { /*#implements*/act(): void {} }
new Base()./*#baseUse*/act();
new Derived()./*#overrideUse*/act();
new Implementation()./*#implementsUse*/act();
type Inline = { /*#inlineLeft*/both: string } | { /*#inlineRight*/both: number };
declare const inline: Inline;
inline./*#inlineUse*/both;
type Missing = { /*#missing*/sometimes: string } | {};
interface Left { /*#interfaceLeft*/both: string; }
interface Right { /*#interfaceRight*/both: number; }
declare const interfaceUnion: Left | Right;
interfaceUnion./*#interfaceUnionUse*/both;
class Sides { static /*#static*/side(): void {} /*#instance*/side(): void {} }
Sides./*#staticUse*/side();
new Sides()./*#instanceUse*/side();
class FirstPrivate { /*#privateFirst*/#key = 1; read() { return this./*#privateFirstUse*/#key; } }
class SecondPrivate { /*#privateSecond*/#key = 1; read() { return this./*#privateSecondUse*/#key; } }
class Accessors { get /*#getter*/item(): string { return 'x'; } set /*#setter*/item(value: string | number) {} }
new Accessors()./*#accessorUse*/item;
