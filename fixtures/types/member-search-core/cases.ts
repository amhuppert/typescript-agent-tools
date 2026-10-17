export {};
interface Box<T> { /*#declaration*/value: T }
declare const first: Box<string>;
declare const second: Box<number>;
first./*#first*/value;
second./*#second*/value;
interface Other { /*#other*/value: string }
declare const other: Other;
other./*#otherUse*/value;
type Inline = { /*#inlineLeft*/both: string } | { /*#inlineRight*/both: number };
declare const inline: Inline;
inline./*#inlineUse*/both;
interface Left { /*#interfaceLeft*/both: string }
interface Right { /*#interfaceRight*/both: number }
declare const combined: Left | Right;
combined./*#interfaceUse*/both;
interface Escaped { /*#escapedDeclaration*/r\u0075n(): void }
declare const escaped: Escaped;
escaped./*#escapedUse*/run();
escaped[/*#literalUse*/"run"]();
interface Numeric { /*#numericDeclaration*/0x10: string }
declare const numeric: Numeric;
numeric[/*#numericUse*/16];
numeric[/*#numericString*/"16"];
declare const arbitrary: any;
arbitrary./*#anyUse*/value;
declare const unknownReceiver: unknown;
unknownReceiver./*#unknownUse*/value;
