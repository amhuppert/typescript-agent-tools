import * as Direct from "./target";
import * as Named from "./named";
import * as Star from "./star";
import JsonShadow from "./value.json";
import { Inside } from "hidden";
declare const /*#directDefault*/directDefault: Direct.Defaulted;
declare const /*#namedDefault*/namedDefault: Named.Defaulted;
declare const /*#starDefault*/starDefault: Star.Defaulted;
declare const /*#directPartial*/directPartial: Direct.Pair<boolean>;
declare const /*#namedPartial*/namedPartial: Named.Pair<boolean>;
function jsonScope() {
  type JsonShadow<T> = { inner: T };
  let /*#json*/json: JsonShadow<"json">;
}
declare const /*#hidden*/hidden: Inside.Carrier<"hidden">;
declare const /*#broken*/broken: Inside.Broken<"broken">;
