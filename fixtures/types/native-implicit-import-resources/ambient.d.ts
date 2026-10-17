declare module "branch-target" {
  class API { static /*#ambientMember*/run(): string; }
  export default API./*#ambientExport*/run;
}
declare module "branch-middle" {
  import * as module from "branch-target";
  export { module as namespaceAlias };
}
declare module "branch-outer" {
  export { namespaceAlias } from "branch-middle";
}
