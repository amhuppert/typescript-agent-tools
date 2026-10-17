export {};
declare const key: "value";
function /*#identifier*/identifier(value: number): void {}
function /*#escapedIdentifier*/escapedIdentifier(v\u0061lue: number): void {}
function /*#object*/object({ value }: { value: number }): void {}
function /*#array*/array([value]: [number]): void {}
function /*#nested*/nested({ outer: [value, { inner }] }: { outer: [number, { inner: string }] }): void {}
function /*#renamed*/renamed({ value: local }: { value: number }): void {}
function /*#singleQuote*/singleQuote({ 'a-b': local }: { 'a-b': number }): void {}
function /*#doubleQuote*/doubleQuote({ "a-b": local }: { "a-b": number }): void {}
function /*#escapedString*/escapedString({ "a\u002db": local }: { "a-b": number }): void {}
function /*#numeric*/numeric({ 0x10: local }: { 16: number }): void {}
function /*#objectDefault*/objectDefault({ value = 1 }: { value?: number } = {}): void {}
function /*#arrayDefault*/arrayDefault([value = 1]: [number?] = []): void {}
function /*#emptyObject*/emptyObject({}: {}): void {}
function /*#emptyArray*/emptyArray([]: []): void {}
function /*#hole*/hole([, value]: [number, string]): void {}
function /*#holes*/holes([,, value]: [number, number, string]): void {}
function /*#terminalHole*/terminalHole([,]: [number]): void {}
function /*#objectComma*/objectComma({ value, }: { value: number }): void {}
function /*#arrayComma*/arrayComma([value,]: [number]): void {}
function /*#arrayRest*/arrayRest([head, ...tail]: [number, ...string[]]): void {}
function /*#computedLiteral*/computedLiteral({ ["value"]: local }: { value: number }): void {}
function /*#computedEntity*/computedEntity({ [key]: local }: { value: number }): void {}
function /*#removedInitializer*/removedInitializer({ value = { [key]: 1 } }: { value?: { value: number } }): void {}
