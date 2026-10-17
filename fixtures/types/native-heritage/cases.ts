// Native instantiateSymbol and getSingleBaseForNonAugmentingSubtype controls.

interface SharedShared { self: this; latent: MissingShared; }
interface SharedA extends SharedShared { marker: number; }
interface SharedB extends SharedShared { marker: number; }
declare const sharedSource: SharedA;
declare function sharedSelect(x: SharedB): "chosen";
declare function sharedSelect(x: SharedA): "same";
const sharedResult = sharedSelect(sharedSource);

interface ParenthesizedShared { self: this; latent: (MissingParenthesized); }
interface ParenthesizedA extends ParenthesizedShared { marker: number; }
interface ParenthesizedB extends ParenthesizedShared { marker: number; }
declare const parenthesizedSource: ParenthesizedA;
declare function parenthesizedSelect(x: ParenthesizedB): "chosen";
declare function parenthesizedSelect(x: ParenthesizedA): "same";
const parenthesizedResult = parenthesizedSelect(parenthesizedSource);

interface NestedShared { self: this; latent: (( /* child */ MissingNested)); }
interface NestedA extends NestedShared { marker: number; }
interface NestedB extends NestedShared { marker: number; }
declare const nestedSource: NestedA;
declare function nestedSelect(x: NestedB): "chosen";
declare function nestedSelect(x: NestedA): "same";
const nestedResult = nestedSelect(nestedSource);

declare const wrappedRoot: ((number));
type WrappedAlias = (({ value: number }));
declare const wrappedAliasRoot: WrappedAlias;

interface UnionShared { self: this; latent: MissingUnion | never; }
interface UnionA extends UnionShared { marker: number; }
interface UnionB extends UnionShared { marker: number; }
declare const unionSource: UnionA;
declare function unionSelect(x: UnionB): "chosen";
declare function unionSelect(x: UnionA): "same";
const unionResult = unionSelect(unionSource);

interface MethodsShared { self: this; latent(): MissingMethods; }
interface MethodsA extends MethodsShared { marker: number; }
interface MethodsB extends MethodsShared { marker: number; }
declare const methodsSource: MethodsA;
declare function methodsSelect(x: MethodsB): "chosen";
declare function methodsSelect(x: MethodsA): "same";
const methodsResult = methodsSelect(methodsSource);

interface MethodParenShared { self: this; latent(): (MissingMethodParen); }
interface MethodParenA extends MethodParenShared { marker: number; }
interface MethodParenB extends MethodParenShared { marker: number; }
declare const methodParenSource: MethodParenA;
declare function methodParenSelect(x: MethodParenB): "chosen";
declare function methodParenSelect(x: MethodParenA): "same";
const methodParenResult = methodParenSelect(methodParenSource);

interface ParameterShared { self: this; latent(x: MissingParameter): void; }
interface ParameterA extends ParameterShared { marker: number; }
interface ParameterB extends ParameterShared { marker: number; }
declare const parameterSource: ParameterA;
declare function parameterSelect(x: ParameterB): "chosen";
declare function parameterSelect(x: ParameterA): "same";
const parameterResult = parameterSelect(parameterSource);

interface GetterShared { self: this; get latent(): MissingGetter; }
interface GetterA extends GetterShared { marker: number; }
interface GetterB extends GetterShared { marker: number; }
declare const getterSource: GetterA;
declare function getterSelect(x: GetterB): "chosen";
declare function getterSelect(x: GetterA): "same";
const getterResult = getterSelect(getterSource);

interface IndependentA { self: this; latent: MissingIndependentSource; }
interface IndependentB { self: this; latent: MissingIndependentTarget; }
declare const independentSource: IndependentA;
declare function independentSelect(x: IndependentB): "chosen";
declare function independentSelect(x: IndependentA): "same";
const independentResult = independentSelect(independentSource);

interface EmptyParentsShared { self: this; latent: (MissingEmptyParents); }
interface EmptyParentsA extends EmptyParentsShared {}
interface EmptyParentsB extends EmptyParentsShared {}
declare const emptyParentsSource: EmptyParentsA;
declare function emptyParentsSelect(x: EmptyParentsB): "chosen";
declare function emptyParentsSelect(x: EmptyParentsA): "same";
const emptyParentsResult = emptyParentsSelect(emptyParentsSource);
