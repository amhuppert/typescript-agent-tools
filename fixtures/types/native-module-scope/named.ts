export interface LocalInterface { value: 11; }
declare const localInterface: LocalInterface;
const /*#exportedInterface*/exportedInterface = localInterface.value;
export type LocalAlias = 12;
declare const /*#exportedAlias*/exportedAlias: LocalAlias;
export declare class LocalClass { value: 13; }
declare const localClass: LocalClass;
const /*#exportedClass*/exportedClass = localClass.value;
export declare const localValue: 14;
const /*#exportedValue*/exportedValue = localValue;
export namespace LocalNamespace { export type Item = 15; }
declare const /*#exportedNamespace*/exportedNamespace: LocalNamespace.Item;
export interface Café { value: 16; }
declare const exactName: Caf\u00e9;
const /*#exactName*/exactNameResult = exactName.value;
export interface lowercase { value: 17; }
declare const caseName: Lowercase;
const /*#caseSensitive*/caseSensitive = caseName.value;
export interface Shadowed { value: 18; }
function localScope() {
    interface Shadowed { value: 19; }
    let local!: Shadowed;
    const /*#localsFirst*/localsFirst = local.value;
}
export import Imported = GlobalTarget;
declare const /*#importEqualsInScope*/importEqualsInScope: Imported.Item;
namespace Internal {
    export interface Own { value: 20; }
    declare const own: Own;
    const /*#namespaceExport*/namespaceExport = own.value;
    export import Imported = GlobalTarget;
    declare const /*#namespaceImportEquals*/namespaceImportEquals: Imported.Item;
}
