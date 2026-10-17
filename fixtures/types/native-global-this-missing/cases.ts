declare let lexicalLet: { hidden: MissingLexicalDisplay };
declare const lexicalConst: number;
declare class LexicalClass { member: MissingClassDisplay; }
interface TypeOnly { member: MissingInterfaceDisplay; }
enum VisibleEnum { Item = 1 }
declare var actualValue: number;
declare function actualFunction(): string;
declare var genuineAny: any;
const missing = globalThis./*#missing*/notPresent;
const lexicalLetRead = globalThis./*#lexicalLet*/lexicalLet;
const lexicalConstRead = globalThis./*#lexicalConst*/lexicalConst;
const lexicalClassRead = globalThis./*#lexicalClass*/LexicalClass;
const typeOnlyRead = globalThis./*#typeOnly*/TypeOnly;
const visibleEnumRead = globalThis./*#visibleEnum*/VisibleEnum;
const actualValueRead = globalThis./*#actualValue*/actualValue;
const actualFunctionRead = globalThis./*#actualFunction*/actualFunction;
const anyRead = genuineAny./*#genuineAny*/notPresent;
const doubleMissing = (globalThis.notPresent, globalThis./*#repeatMissing*/notPresent);
