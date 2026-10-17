declare module "scope-owner" {
    export interface Own { value: 51; }
    export const /*#ambientInterface*/ambientInterface: Own["value"];
    export default interface NamedDefault { value: 52; }
    export const /*#ambientDefault*/ambientDefault: NamedDefault["value"];
    export { remoteValue as AmbientReexport } from "scope-dependency";
    export const /*#ambientPureAlias*/ambientPureAlias: typeof AmbientReexport;
}
