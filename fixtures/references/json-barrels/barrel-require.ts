// A module whose `export =` names a JSON module's value exposes the value's
// properties as its exports, and only them (`getExternalModuleMember`).
import /*@!*/d = require("./data.json");
export = /*@!*/d;
