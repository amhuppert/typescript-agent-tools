export const nested = -/*#number*/1;
export const twice = -(-/*#twice*/1);
export const negativeBig = -/*#big*/0x2an;
export const negated = !/*#boolean*/false;
export const template = /*#template*/`A💡`;
export const key = { /*#propertyKey*/"name": /*#propertyInitializer*/1 };
import { trueValue } from /*#moduleKey*/"../constants/cases";
export const argument = accept(/*#argument*/"ready");
declare function accept(value: string): void;
export const operation = /*#operator*/-1;
export const padded = - /*💡*/ 1;
export const array = [/*#arrayElement*/"array"];
export const conditional = true ? /*#conditional*/"branch" : "other";
export const computed = { [/*#computed*/"member"]: false };
function returned() { return /*#returned*/"result"; }
for (; /*#condition*/true;) { break; }
export const chained = outer(inner(/*#nestedCall*/"nested"));
declare function outer(value: string): string;
declare function inner(value: string): string;
export const constructed = new Made(/*#newArg*/"new");
declare class Made { constructor(value: string); }
export const tagged = tag/*#tagToken*/`tagged`;
declare function tag(value: TemplateStringsArray): string;
export const callback = invoke(() => { const /*#callbackName*/local = 1; return local; });
declare function invoke(value: () => number): number;
export const namedArgument = acceptObject({ /*#callProperty*/"name": 1 });
declare function acceptObject(value: object): void;
