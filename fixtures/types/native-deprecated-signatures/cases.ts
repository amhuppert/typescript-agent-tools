export {};
declare function normal(value: number, hidden?: { bad: MissingNormal }): { value: "normal" };
/** @deprecated */
declare function deprecated(value: number, hidden?: { bad: MissingDeprecated }): { value: "deprecated" };
const normalCase = normal(1)./*#normal*/value;
const deprecatedCase = deprecated(1)./*#deprecated*/value;
const quickNormal = normal(1);
const quickDeprecated = deprecated(1);
/*#quickNormal*/quickNormal;
/*#quickDeprecated*/quickDeprecated;
declare function normalDefault(value: number, hidden?: <T = MissingNormalDefault>() => void): { value: "normalDefault" };
/** @deprecated */
declare function deprecatedDefault(value: number, hidden?: <T = MissingDeprecatedDefault>() => void): { value: "deprecatedDefault" };
const normalDefaultCase = normalDefault(1)./*#normalDefault*/value;
const deprecatedDefaultCase = deprecatedDefault(1)./*#deprecatedDefault*/value;
/** @deprecated */
declare function healthy(value: number, hidden?: { safe: string }): { value: "healthy" };
const healthyCase = healthy(1)./*#healthy*/value;
/** @deprecatedMore */
declare function notDeprecated(value: number, hidden?: { bad: MissingNotDeprecated }): { value: "notDeprecated" };
const notDeprecatedCase = notDeprecated(1)./*#notDeprecated*/value;
declare class NormalClass { readonly value: "normalClass"; constructor(value: number, hidden?: { bad: MissingNormalClass }); }
declare class DeprecatedClass {
 readonly value: "deprecatedClass";
 /** @deprecated */
 constructor(value: number, hidden?: { bad: MissingDeprecatedClass });
}
const normalClassCase = new NormalClass(1)./*#normalClass*/value;
const deprecatedClassCase = new DeprecatedClass(1)./*#deprecatedClass*/value;
const quickDeprecatedClass = /*#quickDeprecatedClass*/new DeprecatedClass(1);
declare function normalNew(): MissingNormalNew;
/** @deprecated */
declare function deprecatedNew(): MissingDeprecatedNew;
const normalNewCase = /*#normalNew*/new normalNew();
const deprecatedNewCase = /*#deprecatedNew*/new deprecatedNew();
/** @deprecated */
declare function overload(value: string, hidden?: { bad: MissingOverload }): { value: "string" };
declare function overload(value: number): { value: "number" };
const overloadNormalCase = overload(1)./*#overloadNormal*/value;
const overloadDeprecatedCase = overload("x")./*#overloadDeprecated*/value;
/** `@deprecated` */
declare function inlineCode(value: number, hidden?: { bad: MissingInlineCode }): { value: "inlineCode" };
const inlineCodeCase = inlineCode(1)./*#inlineCode*/value;
/**
 * ```ts
 * @deprecated
 * ```
 */
declare function fencedCode(value: number, hidden?: { bad: MissingFencedCode }): { value: "fencedCode" };
const fencedCodeCase = fencedCode(1)./*#fencedCode*/value;
/** @deprecated-more */
declare function hyphenTag(value: number, hidden?: { bad: MissingHyphenTag }): { value: "hyphenTag" };
const hyphenTagCase = hyphenTag(1)./*#hyphenTag*/value;
/** user@deprecated */
declare function emailText(value: number, hidden?: { bad: MissingEmailText }): { value: "emailText" };
const emailTextCase = emailText(1)./*#emailText*/value;
/** @deprecated message */
declare function nbspTag(value: number, hidden?: { bad: MissingNbspTag }): { value: "nbspTag" };
const nbspTagCase = nbspTag(1)./*#nbspTag*/value;
/** {@link deprecated @deprecated} */
declare function linkText(value: number, hidden?: { bad: MissingLinkText }): { value: "linkText" };
const linkTextCase = linkText(1)./*#linkText*/value;
/**
 * @param value ignored
 * @deprecated real tag after another tag
 */
declare function secondTag(value: number, hidden?: { bad: MissingSecondTag }): { value: "secondTag" };
const secondTagCase = secondTag(1)./*#secondTag*/value;
/** @deprecated */
declare function tag(strings: TemplateStringsArray, hidden?: { bad: MissingTag }): { value: "tag" };
const tagCase = tag`text`./*#tag*/value;
