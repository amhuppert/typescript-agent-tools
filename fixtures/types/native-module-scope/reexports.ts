export { remoteValue as ReexportValue, RemoteType as ReexportType } from "./target";
export * as ReexportNamespace from "./target";
export * from "./target";
const /*#pureValueAlias*/pureValueAlias = ReexportValue;
declare const reexportType: ReexportType;
const /*#pureTypeAlias*/pureTypeAlias = reexportType.value;
const /*#pureNamespaceAlias*/pureNamespaceAlias = ReexportNamespace.value;
const /*#starIsNotScope*/starIsNotScope = StarValue;
export interface Mixed { value: 41; }
export { remoteValue as Mixed } from "./target";
declare const mixed: Mixed;
const /*#mixedAliasRetained*/mixedAliasRetained = mixed.value;
const /*#mixedAliasValue*/mixedAliasValue = Mixed;
