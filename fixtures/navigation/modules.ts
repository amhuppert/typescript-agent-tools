// Import and export forms.
import def, { named as alias, type Only } from "./other";
import * as starNs from "./other";
import "./side-effect";
import Req = require("./other");
export import Reexported = Req.inner;

const value = 1;
export { value as exportedValue, value as "string name" };
export { value };
export * from "./other";
export * as allOfIt from "./other";
export type { Only as OnlyType };

export default function () {
  const insideDefault = 1;
}
