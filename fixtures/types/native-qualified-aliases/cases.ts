import * as Direct from "./target";
import * as Reexport from "./named";
import * as AliasFirst from "./alias-first";
import * as DirectFirst from "./direct-first";
import * as Cycle from "./cycle-a";
import Equals = require("./export-equals");
import { Original as Shadow } from "./target";
import { Missing as MissingShadow } from "./target";
import type { External as DependencyShadow } from "dependency";
namespace Local {
  export type Own<T> = { own: T };
  export import Imported = Direct.Original;
}
declare const /*#direct*/direct: Direct.Original<"direct">;
declare const /*#named*/named: Reexport.Named<"named">;
declare const /*#typeOnly*/typeOnly: Reexport.TypeOnly<"typeOnly">;
declare const /*#nested*/nested: Reexport.Nested.Original<"nested">;
declare const /*#aliasFirst*/aliasFirst: AliasFirst.Original<"first">;
declare const /*#directFirst*/directFirst: DirectFirst.Original<"first">;
declare const /*#cycle*/cycle: Cycle.Named<"cycle">;
declare const /*#equals*/equals: Equals.Original<"equals">;
declare const /*#own*/own: Local.Own<"own">;
declare const /*#localAlias*/localAlias: Local.Imported<"alias">;
function shadowScope() {
  type Shadow<T> = { inner: T };
  let /*#shadow*/shadow: Shadow<"shadow">;
}
function dependencyScope() {
  type DependencyShadow<T> = { inner: T };
  let /*#dependency*/dependency: DependencyShadow<"dependency">;
}
function missingScope() {
  type MissingShadow<T> = { inner: T };
  let /*#missing*/missing: MissingShadow<"missing">;
}
