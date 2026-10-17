export {};
declare const aliasKey: "Alias";
declare const interfaceKey: "Shape";
declare class Mixed {
    static [aliasKey]: 81;
    static [interfaceKey]: 82;
}
declare namespace Mixed {
    type Alias = 83;
    interface Shape { member: 84; }
}
const /*#mixedAliasValue*/mixedAliasValue = Mixed.Alias;
declare const aliasType: Mixed.Alias;
const /*#mixedAliasType*/mixedAliasType = aliasType;
const /*#mixedInterfaceValue*/mixedInterfaceValue = Mixed.Shape;
declare const interfaceType: Mixed.Shape;
const /*#mixedInterfaceType*/mixedInterfaceType = interfaceType.member;
