export {};
class /*#plainClass*/Plain { value: number; }
class /*#genericClass*/Generic<T> { value: T; }
class /*#derivedClass*/Derived extends Plain { own: string; }
const expressionClass = class /*#expressionClass*/Inside { value: boolean; };
const genericExpressionClass = class /*#genericExpressionClass*/InsideGeneric<T> { value: T; };
export default class /*#defaultClass*/NamedDefault { value: string; }
declare let plain: Plain;
declare let generic: Generic<any>;
Plain./*#prototypeRead*/prototype;
Plain./*#prototypeWrite*/prototype = plain;
Generic./*#genericPrototypeWrite*/prototype = generic;

class ReadonlyPrototype { static readonly prototype: ReadonlyPrototype; value: number; }
class GetterPrototype { static get prototype(): GetterPrototype { return undefined as any; } value: string; }
class AccessorPrototype {
    static get prototype(): AccessorPrototype { return undefined as any; }
    static set prototype(value: AccessorPrototype) {}
    value: boolean;
}
declare let readonlyPrototype: ReadonlyPrototype;
declare let getterPrototype: GetterPrototype;
declare let accessorPrototype: AccessorPrototype;
ReadonlyPrototype./*#readonlyPrototypeWrite*/prototype = readonlyPrototype;
GetterPrototype./*#getterPrototypeWrite*/prototype = getterPrototype;
AccessorPrototype./*#accessorPrototypeWrite*/prototype = accessorPrototype;
declare let /*#plainStatic*/plainStatic: typeof Plain;
declare let /*#readonlyStatic*/readonlyStatic: typeof ReadonlyPrototype;
declare let /*#getterStatic*/getterStatic: typeof GetterPrototype;
declare let /*#accessorStatic*/accessorStatic: typeof AccessorPrototype;
class AnnotatedPrototype { static prototype: string; value: number; }
class GetterStringPrototype { static get prototype(): string { return ""; } }
class AccessorStringPrototype {
    static get prototype(): string { return ""; }
    static set prototype(value: boolean) {}
}
AnnotatedPrototype./*#annotatedPrototypeRead*/prototype;
GetterStringPrototype./*#getterStringRead*/prototype;
AccessorStringPrototype./*#accessorStringRead*/prototype;
AccessorStringPrototype./*#accessorStringWrite*/prototype = true;
declare let /*#annotatedStatic*/annotatedStatic: typeof AnnotatedPrototype;
declare let /*#getterStringStatic*/getterStringStatic: typeof GetterStringPrototype;
declare let /*#accessorStringStatic*/accessorStringStatic: typeof AccessorStringPrototype;

interface PropertyFirst { readonly value: string; }
interface PropertyFirst { set value(input: boolean); }
interface SetterFirst { set value(input: boolean); }
interface SetterFirst { readonly value: string; }
interface WithGetter { readonly value: string; }
interface WithGetter { get value(): string; set value(input: boolean); }
interface GetterOnly { readonly value: string; }
interface GetterOnly { get value(): string; }
interface Mutable { value: string; }
interface Mutable { set value(input: boolean); }
interface GenericReadonly<T> { readonly value: T; }
interface GenericReadonly<T> { set value(input: T); }
declare let propertyFirst: PropertyFirst;
declare let setterFirst: SetterFirst;
declare let withGetter: WithGetter;
declare let getterOnly: GetterOnly;
declare let mutable: Mutable;
declare let instantiated: GenericReadonly<number>;
propertyFirst./*#propertyFirstWrite*/value = true;
setterFirst./*#setterFirstWrite*/value = true;
withGetter./*#withGetterWrite*/value = "x";
getterOnly./*#getterOnlyWrite*/value = "x";
mutable./*#mutableWrite*/value = true;
instantiated./*#genericReadonlyWrite*/value = 1;
