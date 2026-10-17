export {};
enum SourceHigh { "\uD800" = 1 }
enum ResultHigh { Member = SourceHigh[/*@key-High*/"\uD800"] }
declare const /*#keyHigh*/chosenHigh: ResultHigh.Member extends 1 ? "one" : "other";
enum SourceLow { "\uDC00" = 1 }
enum ResultLow { Member = SourceLow[/*@key-Low*/"\uDC00"] }
declare const /*#keyLow*/chosenLow: ResultLow.Member extends 1 ? "one" : "other";
enum SourceLF { "\uD800\
\uDC00" = 1 }
enum ResultLF { Member = SourceLF[/*@key-LF*/"\uD800\
\uDC00"] }
declare const /*#keyLF*/chosenLF: ResultLF.Member extends 1 ? "one" : "other";
enum SourceCR { "\uD800\\uDC00" = 1 }
enum ResultCR { Member = SourceCR[/*@key-CR*/"\uD800\\uDC00"] }
declare const /*#keyCR*/chosenCR: ResultCR.Member extends 1 ? "one" : "other";
enum SourceCRLF { "\uD800\
\uDC00" = 1 }
enum ResultCRLF { Member = SourceCRLF[/*@key-CRLF*/"\uD800\
\uDC00"] }
declare const /*#keyCRLF*/chosenCRLF: ResultCRLF.Member extends 1 ? "one" : "other";
enum SourceLS { "\uD800\ \uDC00" = 1 }
enum ResultLS { Member = SourceLS[/*@key-LS*/"\uD800\ \uDC00"] }
declare const /*#keyLS*/chosenLS: ResultLS.Member extends 1 ? "one" : "other";
enum SourcePS { "\uD800\ \uDC00" = 1 }
enum ResultPS { Member = SourcePS[/*@key-PS*/"\uD800\ \uDC00"] }
declare const /*#keyPS*/chosenPS: ResultPS.Member extends 1 ? "one" : "other";
enum SourcePair { "\uD800\uDC00" = 1 }
enum ResultPair { Member = SourcePair[/*@key-Pair*/"\uD800\uDC00"] }
declare const /*#keyPair*/chosenPair: ResultPair.Member extends 1 ? "one" : "other";
enum SourceScalar { "𐀀" = 1 }
enum ResultScalar { Member = SourceScalar[/*@key-Scalar*/"𐀀"] }
declare const /*#keyScalar*/chosenScalar: ResultScalar.Member extends 1 ? "one" : "other";
enum ResultTemplateHigh { Member = SourceHigh[/*@key-TemplateHigh*/`\uD800`] }
declare const /*#keyTemplateHigh*/chosenTemplateHigh: ResultTemplateHigh.Member extends 1 ? "one" : "other";
enum ResultTemplateLow { Member = SourceLow[/*@key-TemplateLow*/`\uDC00`] }
declare const /*#keyTemplateLow*/chosenTemplateLow: ResultTemplateLow.Member extends 1 ? "one" : "other";
enum ResultTemplateLF { Member = SourceLF[/*@key-TemplateLF*/`\uD800\
\uDC00`] }
declare const /*#keyTemplateLF*/chosenTemplateLF: ResultTemplateLF.Member extends 1 ? "one" : "other";
enum ResultTemplateCR { Member = SourceCR[/*@key-TemplateCR*/`\uD800\\uDC00`] }
declare const /*#keyTemplateCR*/chosenTemplateCR: ResultTemplateCR.Member extends 1 ? "one" : "other";
enum ResultTemplateCRLF { Member = SourceCRLF[/*@key-TemplateCRLF*/`\uD800\
\uDC00`] }
declare const /*#keyTemplateCRLF*/chosenTemplateCRLF: ResultTemplateCRLF.Member extends 1 ? "one" : "other";
enum ResultTemplateLS { Member = SourceLS[/*@key-TemplateLS*/`\uD800\ \uDC00`] }
declare const /*#keyTemplateLS*/chosenTemplateLS: ResultTemplateLS.Member extends 1 ? "one" : "other";
enum ResultTemplatePS { Member = SourcePS[/*@key-TemplatePS*/`\uD800\ \uDC00`] }
declare const /*#keyTemplatePS*/chosenTemplatePS: ResultTemplatePS.Member extends 1 ? "one" : "other";
enum ResultTemplatePair { Member = SourcePair[/*@key-TemplatePair*/`\uD800\uDC00`] }
declare const /*#keyTemplatePair*/chosenTemplatePair: ResultTemplatePair.Member extends 1 ? "one" : "other";
enum ResultTemplateScalar { Member = SourceScalar[/*@key-TemplateScalar*/`𐀀`] }
declare const /*#keyTemplateScalar*/chosenTemplateScalar: ResultTemplateScalar.Member extends 1 ? "one" : "other";
