export {};
const high = /*#highExpression*/"\uD800";
const low = /*#lowExpression*/"\uDC00";
declare const /*#highDeclaration*/highDeclared: "\uD800";
type /*#highType*/High = "\uD800";
const highTemplate = /*#highTemplate*/`\uD800`;
const reverse = /*#reversedHalves*/"\uDC00\uD800";
const fixedFixed = /*#fixedFixed*/"\uD800\uDC00";
const fixedBraced = /*#fixedBraced*/"\uD800\u{DC00}";
const bracedFixed = /*#bracedFixed*/"\u{D800}\uDC00";
const bracedBraced = /*#bracedBraced*/"\u{D800}\u{DC00}";
const scalar = /*#scalar*/"𐀀";
const replacement = /*#replacement*/"�";
const lf = /*#continuedLF*/"\uD800\
\uDC00";
declare const /*#continuedLFEquality*/lfEquality: typeof lf extends "𐀀" ? true : false;
const cr = /*#continuedCR*/"\uD800\\uDC00";
declare const /*#continuedCREquality*/crEquality: typeof cr extends "𐀀" ? true : false;
const crlf = /*#continuedCRLF*/"\uD800\
\uDC00";
declare const /*#continuedCRLFEquality*/crlfEquality: typeof crlf extends "𐀀" ? true : false;
const ls = /*#continuedLS*/"\uD800\ \uDC00";
declare const /*#continuedLSEquality*/lsEquality: typeof ls extends "𐀀" ? true : false;
const ps = /*#continuedPS*/"\uD800\ \uDC00";
declare const /*#continuedPSEquality*/psEquality: typeof ps extends "𐀀" ? true : false;
const /*#constantTemplate*/constantTemplate = `${"\uD800"}${"\uDC00"}`;
declare const /*#constantTemplateEquality*/constantTemplateEquality: typeof constantTemplate extends "𐀀" ? true : false;
type /*#typeTemplate*/TypeTemplate = `${"\uD800"}${"\uDC00"}`;
type /*#continuedTypeTemplate*/ContinuedTypeTemplate = `${typeof lf}`;
declare const /*#mappings*/mappings: [Uppercase<"\uD800a">, Lowercase<"\uDC00A">, Capitalize<"\uD800a">, Uncapitalize<"\uDC00A">];
declare const /*#expansionAndSigma*/expansionAndSigma: [Uppercase<"\uD800ß">, Lowercase<"AΣ\uD800">, Uppercase<typeof lf>];
enum SourceEnum { /*#enumHigh*/High = "\uD800", /*#enumLow*/Low = "\uDC00", /*#enumPair*/Pair = "\uD800\uDC00", /*#enumConcat*/Concat = "\uD800" + "\uDC00" }
type /*#enumConcatTemplate*/EnumConcatTemplate = `${SourceEnum.Concat}`;
type /*#enumHighTemplate*/EnumHighTemplate = `${SourceEnum.High}`;
