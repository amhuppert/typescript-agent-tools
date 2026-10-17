import Ambient = require(/*#ambientImport*/"ambient");
import { /*#importLiteral*/"length" as importedLength } from "./exported";
const local = 1;
export { local as /*#exportLiteral*/"length" };
type Length = /*#objectLiteral*/"length"[/*#indexLiteral*/"length"];
declare const value: string;
value./*#lengthUse*/length;
const ordinary = /*#ordinaryLiteral*/"length";
const target = {};
Object.defineProperty(target, /*#defineProperty*/"length", {});
Object.defineProperty(target, /*#definePropertyWrongArity*/"length");
Object.defineProperty((target), /*#definePropertyParenthesized*/"length", {});
namespace /*#constNamespace*/ConstOnly { export const enum E { A } }
type ConstType = /*#constTypeUse*/ConstOnly.E;
const constValue = /*#constValueUse*/ConstOnly.E.A;
namespace /*#valueNamespace*/Live { export const enum E { A } export const runtime = 1; }
type LiveType = /*#liveTypeUse*/Live.E;
const liveValue = /*#liveValueUse*/Live.runtime;
