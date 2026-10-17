import /*#equalsAlias*/execute = require("./factory");
/*#equalsCall*/execute();
const saved = /*#equalsSaved*/execute;
import /*#defaultAlias*/invoke from "./default";
/*#defaultCall*/invoke();
import * as module from "./default";
module./*#namespaceCall*/default();
import { /*#namedSource*/rerun as /*#namedAlias*/renamed } from "./barrel";
/*#renamedCall*/renamed();
