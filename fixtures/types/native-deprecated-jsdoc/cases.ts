export {};
/** @deprecated message */
declare function fn_space(value: number, hidden?: { bad: Missing_space }): { value: "space" };
const use_space = fn_space(1)./*#space*/value;
/** @deprecated	message */
declare function fn_tab(value: number, hidden?: { bad: Missing_tab }): { value: "tab" };
const use_tab = fn_tab(1)./*#tab*/value;
/** @deprecatedmessage */
declare function fn_cr(value: number, hidden?: { bad: Missing_cr }): { value: "cr" };
const use_cr = fn_cr(1)./*#cr*/value;
/** @deprecated
message */
declare function fn_lf(value: number, hidden?: { bad: Missing_lf }): { value: "lf" };
const use_lf = fn_lf(1)./*#lf*/value;
/** @deprecated
message */
declare function fn_crlf(value: number, hidden?: { bad: Missing_crlf }): { value: "crlf" };
const use_crlf = fn_crlf(1)./*#crlf*/value;
/** @deprecated} */
declare function fn_brace(value: number, hidden?: { bad: Missing_brace }): { value: "brace" };
const use_brace = fn_brace(1)./*#brace*/value;
/** @deprecated* */
declare function fn_asterisk(value: number, hidden?: { bad: Missing_asterisk }): { value: "asterisk" };
const use_asterisk = fn_asterisk(1)./*#asterisk*/value;
/** @deprecated*/
declare function fn_bare(value: number, hidden?: { bad: Missing_bare }): { value: "bare" };
const use_bare = fn_bare(1)./*#bare*/value;
/** @deprecated message */
declare function fn_nbsp(value: number, hidden?: { bad: Missing_nbsp }): { value: "nbsp" };
const use_nbsp = fn_nbsp(1)./*#nbsp*/value;
/** @deprecatedmessage */
declare function fn_vt(value: number, hidden?: { bad: Missing_vt }): { value: "vt" };
const use_vt = fn_vt(1)./*#vt*/value;
/** @deprecatedmessage */
declare function fn_ff(value: number, hidden?: { bad: Missing_ff }): { value: "ff" };
const use_ff = fn_ff(1)./*#ff*/value;
/** @deprecated message */
declare function fn_ls(value: number, hidden?: { bad: Missing_ls }): { value: "ls" };
const use_ls = fn_ls(1)./*#ls*/value;
/** @deprecated message */
declare function fn_ps(value: number, hidden?: { bad: Missing_ps }): { value: "ps" };
const use_ps = fn_ps(1)./*#ps*/value;
/** @see Object
 * @deprecated message */
declare function fn_eagerNbsp(value: number, hidden?: { bad: Missing_eagerNbsp }): { value: "eagerNbsp" };
const use_eagerNbsp = fn_eagerNbsp(1)./*#eagerNbsp*/value;
/** @see Object
 * @deprecat\u0065d message */
declare function fn_eagerEscaped(value: number, hidden?: { bad: Missing_eagerEscaped }): { value: "eagerEscaped" };
const use_eagerEscaped = fn_eagerEscaped(1)./*#eagerEscaped*/value;
/** @deprecat\u0065d message */
declare function fn_escaped(value: number, hidden?: { bad: Missing_escaped }): { value: "escaped" };
const use_escaped = fn_escaped(1)./*#escaped*/value;
/** @param {Array<{ nested: string }>} value text
 * @deprecated */
declare function fn_param(value: number, hidden?: { bad: Missing_param }): { value: "param" };
const use_param = fn_param(1)./*#param*/value;
/** @param [value={x:1}] text
 * @deprecated */
declare function fn_paramDefault(value: number, hidden?: { bad: Missing_paramDefault }): { value: "paramDefault" };
const use_paramDefault = fn_paramDefault(1)./*#paramDefault*/value;
/** @param `value` text
 * @deprecated */
declare function fn_paramQuoted(value: number, hidden?: { bad: Missing_paramQuoted }): { value: "paramQuoted" };
const use_paramQuoted = fn_paramQuoted(1)./*#paramQuoted*/value;
/** @param {Object} value
 * @param {string} value.x text
 * @deprecated */
declare function fn_paramNested(value: number, hidden?: { bad: Missing_paramNested }): { value: "paramNested" };
const use_paramNested = fn_paramNested(1)./*#paramNested*/value;
/** @template {Object} T, [U=Array<string>]
 * @deprecated */
declare function fn_template(value: number, hidden?: { bad: Missing_template }): { value: "template" };
const use_template = fn_template(1)./*#template*/value;
/** @typedef {Object} Name
 * @property {string} x
 * @deprecated */
declare function fn_typedef(value: number, hidden?: { bad: Missing_typedef }): { value: "typedef" };
const use_typedef = fn_typedef(1)./*#typedef*/value;
/** @typedef Name
 * @type {string}
 * @deprecated */
declare function fn_typedefType(value: number, hidden?: { bad: Missing_typedefType }): { value: "typedefType" };
const use_typedefType = fn_typedefType(1)./*#typedefType*/value;
/** @callback Name
 * @param {string} value
 * @returns {string}
 * @deprecated */
declare function fn_callback(value: number, hidden?: { bad: Missing_callback }): { value: "callback" };
const use_callback = fn_callback(1)./*#callback*/value;
/** @overload
 * @param {string} value
 * @deprecated */
declare function fn_overload(value: number, hidden?: { bad: Missing_overload }): { value: "overload" };
const use_overload = fn_overload(1)./*#overload*/value;
/** @implements {Object}
 * @deprecated */
declare function fn_implements(value: number, hidden?: { bad: Missing_implements }): { value: "implements" };
const use_implements = fn_implements(1)./*#implements*/value;
/** @augments Object<string>
 * @deprecated */
declare function fn_augments(value: number, hidden?: { bad: Missing_augments }): { value: "augments" };
const use_augments = fn_augments(1)./*#augments*/value;
/** @import { Array as A } from "missing-doc-module"
 * @deprecated */
declare function fn_importCase(value: number, hidden?: { bad: Missing_import }): { value: "import" };
const use_import = fn_importCase(1)./*#import*/value;
/** @import A, { B } from "missing-doc-module"
 * @deprecated */
declare function fn_importDefault(value: number, hidden?: { bad: Missing_importDefault }): { value: "importDefault" };
const use_importDefault = fn_importDefault(1)./*#importDefault*/value;
/** @import * as A from "missing-doc-module"
 * @deprecated */
declare function fn_importNamespace(value: number, hidden?: { bad: Missing_importNamespace }): { value: "importNamespace" };
const use_importNamespace = fn_importNamespace(1)./*#importNamespace*/value;
/** @satisfies {Object}
 * @deprecated */
declare function fn_satisfies(value: number, hidden?: { bad: Missing_satisfies }): { value: "satisfies" };
const use_satisfies = fn_satisfies(1)./*#satisfies*/value;
/** @throws {Error} text
 * @deprecated */
declare function fn_throws(value: number, hidden?: { bad: Missing_throws }): { value: "throws" };
const use_throws = fn_throws(1)./*#throws*/value;
/** @this {Object}
 * @deprecated */
declare function fn_thisCase(value: number, hidden?: { bad: Missing_this }): { value: "this" };
const use_this = fn_thisCase(1)./*#this*/value;
/** @type {function(string): string}
 * @deprecated */
declare function fn_type(value: number, hidden?: { bad: Missing_type }): { value: "type" };
const use_type = fn_type(1)./*#type*/value;
/** @param {string value
 * @deprecated */
declare function fn_unclosedType(value: number, hidden?: { bad: Missing_unclosedType }): { value: "unclosedType" };
const use_unclosedType = fn_unclosedType(1)./*#unclosedType*/value;
/** @param [value=
 * @deprecated */
declare function fn_unclosedDefault(value: number, hidden?: { bad: Missing_unclosedDefault }): { value: "unclosedDefault" };
const use_unclosedDefault = fn_unclosedDefault(1)./*#unclosedDefault*/value;
/** user@deprecated */
declare function fn_email(value: number, hidden?: { bad: Missing_email }): { value: "email" };
const use_email = fn_email(1)./*#email*/value;
/** `@deprecated` */
declare function fn_inline(value: number, hidden?: { bad: Missing_inline }): { value: "inline" };
const use_inline = fn_inline(1)./*#inline*/value;
/**
 * ```ts
 * @deprecated
 * ```
 */
declare function fn_fence(value: number, hidden?: { bad: Missing_fence }): { value: "fence" };
const use_fence = fn_fence(1)./*#fence*/value;
/** {@link Object @deprecated} */
declare function fn_link(value: number, hidden?: { bad: Missing_link }): { value: "link" };
const use_link = fn_link(1)./*#link*/value;
/** @deprecatedMore */
declare function fn_long(value: number, hidden?: { bad: Missing_long }): { value: "long" };
const use_long = fn_long(1)./*#long*/value;
/** @deprecated-more */
declare function fn_hyphen(value: number, hidden?: { bad: Missing_hyphen }): { value: "hyphen" };
const use_hyphen = fn_hyphen(1)./*#hyphen*/value;
declare const typed: /** @deprecated */ (value: number, hidden?: { bad: Missing_typed }) => { value: "typed" };
const use_typed = typed(1)./*#typed*/value;
declare const constructed: /** @deprecated */ new (value: number, hidden?: { bad: Missing_constructed }) => { value: "constructed" };
const use_constructed = new constructed(1)./*#constructed*/value;
declare const callable: { /** @deprecated */ (value: number, hidden?: { bad: Missing_callable }): { value: "callable" }; };
const use_callable = callable(1)./*#callable*/value;
const arrow = /** @deprecated */ (value: number, hidden?: { bad: Missing_arrow }): {value:"arrow"} => ({value:"arrow"});
const use_arrow = arrow(1)./*#arrow*/value;
const fn = /** @deprecated */ function(value: number, hidden?: {bad:Missing_fn}): {value:"fn"} {return {value:"fn"}};
const use_fn = fn(1)./*#fn*/value;
const method = { /** @deprecated */ f(value:number, hidden?:{bad:Missing_method}): {value:"method"} {return {value:"method"}} };
const use_method = method.f(1)./*#method*/value;
declare class Keys<T = MissingQualifiedDefault> { static readonly key: unique symbol; }
/** @deprecated */
declare function fn_qualified(hidden?: { [Keys.key]: string }): { value: "qualified" };
const use_qualified = fn_qualified()./*#qualified*/value;
declare function fn_qualifiedNormal(hidden?: { [Keys.key]: string }): { value: "qualifiedNormal" };
const use_qualifiedNormal = fn_qualifiedNormal()./*#qualifiedNormal*/value;
