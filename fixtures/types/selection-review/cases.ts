import { /*#classImportLeft*/Widget as /*#classImportRight*/ImportedWidget } from './exports';
import type { /*#typeClassLeft*/Widget as /*#typeClassRight*/TypeWidget } from './exports';
import { type /*#specifierTypeLeft*/Widget as /*#specifierTypeRight*/SpecifierWidget } from './exports';
import { /*#interfaceLeft*/Shape as /*#interfaceRight*/ImportedShape } from './exports';
import type { /*#typeFunctionLeft*/fn as /*#typeFunctionRight*/TypeFunction, /*#typeVariableLeft*/constant as /*#typeVariableRight*/TypeVariable } from './exports';
import /*#defaultOrdinary*/DefaultOrdinary from './exports';
import type /*#defaultType*/DefaultType from './exports';
import * as /*#namespaceImport*/Everything from './exports';
import type * as /*#typeNamespaceImport*/TypesOnly from './types-only';
import /*#importEquals*/Equal = require('./equals.cjs');
import json from './keys.json';
export { /*#exportLeft*/Widget as /*#exportRight*/ExportedWidget } from './exports';
export type { /*#typeExportLeft*/Widget as /*#typeExportRight*/ExportedTypeWidget } from './exports';
export { type /*#specifierExportLeft*/Widget as /*#specifierExportRight*/SpecifierExportedWidget } from './exports';
export type { /*#functionExportLeft*/fn as /*#functionExportRight*/ExportedTypeFunction, /*#variableExportLeft*/constant as /*#variableExportRight*/ExportedTypeVariable } from './exports';
class /*#classDeclaration*/Local { n = 1; static label = 'local'; }
const constructorValue = /*#classValue*/Local;
enum /*#enumDeclaration*/LocalState { /*#enumMember*/Ready = 1 }
const enumValue = /*#enumValue*/LocalState;
namespace /*#activeNamespace*/Active { export const n = 1; export interface Box<T> { value: T } }
namespace /*#passiveNamespace*/Passive { export interface I { value: number } }
import /*#internalEquals*/InternalEqual = Active;
type Query = typeof /*#typeQuery*/Local;
type QualifiedQuery = typeof Active./*#qualifiedTypeQuery*/n;
type Instantiated = Active./*#typeReference*/Box<string>;
type Earlier = /*#earlierNamespace*/Active.Box<number>;
type Unknown = /*#unresolvedType*/Missing;
const literal = 'fixed';
const shorthand = { /*#shorthand*/literal };
class ParameterHost {
  constructor(/*#parameterModifier*/public /*#parameterName*/value: string) {}
  get /*#getter*/count(): number { return 1; }
  set /*#setter*/count(value: number | string) {}
}
declare const object: { n: number };
object[/*#elementKey*/'n'/*#elementClose*/];
function call(value: number): 'result' { return 'result'; }
/*#callee*/call(1/*#callClose*/);
declare function tag(parts: TemplateStringsArray): 'tagged';
tag`text/*#tagClose*/`;
/*#newKeyword*/new Local;
type CallShape = { /*#callSignature*/(n: number): string; /*#constructSignature*/new (n: number): Local; /*#indexSignature*/[key: string]: unknown };
class ConstructorHost { /*#constructorKeyword*/constructor() {} }
namespace WithBlock /*#moduleBlock*/{ export const n = 1; }
export default /*#defaultExpression*/42;
export = /*#equalsExpression*/call(1);
