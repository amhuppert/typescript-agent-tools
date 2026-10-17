export {};
declare const staticKey: "staticValue";
declare class Merged {
    static [staticKey]: 51;
}
declare namespace Merged {
    interface Item { value: 52; }
}
const /*#mergedStatic*/mergedStatic = Merged.staticValue;
declare const item: Merged.Item;
const /*#mergedNamespaceType*/mergedNamespaceType = item.value;
declare const staticType: typeof Merged.staticValue;
const /*#mergedQualifiedTypeof*/mergedQualifiedTypeof = staticType;

declare const accessorKey: "value";
class AccessorPair {
    get [accessorKey]() { return 61 as const; }
    set [accessorKey](input) {
        const /*#lateAccessorParameter*/lateAccessorParameter = input;
    }
}
class EarlyGetterPair {
    get value() { return 62 as const; }
    set [accessorKey](input) {
        const /*#earlyGetterLateSetterParameter*/earlyGetterLateSetterParameter = input;
    }
}
class LateGetterPair {
    get [accessorKey]() { return 63 as const; }
    set value(input) {
        const /*#lateGetterEarlySetterParameter*/lateGetterEarlySetterParameter = input;
    }
}

declare const anyCall: any;
const /*#accessorSourceCheck*/accessorSourceCheck = anyCall<{
    get [accessorKey](): number;
    set [accessorKey](value: number | string);
}>();
const /*#accessorSourceRecovery*/accessorSourceRecovery = anyCall<{
    get [accessorKey](): MissingGetterAnnotation;
    set [accessorKey](value: number);
}>();

const Anonymous = class {
    static [staticKey] = 64 as const;
};
const /*#classExpressionStatic*/classExpressionStatic = Anonymous.staticValue;
