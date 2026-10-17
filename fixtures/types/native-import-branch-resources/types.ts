export type EqualsModule = typeof import("./factory");
export type DefaultModule = typeof import("./default");
export type QualifiedDefault = typeof import("./default").default;
import * as module from "./default";
export { module as namespaceAlias };
export type DeepQualified = typeof import("./default").Branch.Nested.marker;
