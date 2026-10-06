// A module object is a value and a namespace, so an `import =` alias of a
// namespace import or `require` has both meanings, as does an alias of that.
import * as /*@m:i*/ns from "./m.js";
import /*@m:i*/x = /*@m:u*/ns;
import /*@m:i*/y = /*@m:u*/x;
/*@m:u*/x./*@foo:u*/foo(); /*@m:u*/y./*@foo:u*/foo();
let /*@i:d*/i: /*@m:u*/x./*@I:u*/I = {};

import /*@m:i*/r = require("./m.js");
import /*@m:i*/s = /*@m:u*/r;
/*@m:u*/s./*@foo:u*/foo();

// An inner type does not shadow an outer value of the same name.
{
  type /*@xType:d*/x = number;
  /*@m:u*/x./*@foo:u*/foo();
  let /*@w:d*/w: /*@xType:u*/x = 1;
  /*@w:u*/w;
}
function /*@f:d*/f() {
  interface /*@nsType:d*/ns {}
  /*@m:u*/ns./*@foo:u*/foo();
  let /*@q:d*/q: /*@nsType:u*/ns = {};
  return /*@q:u*/q;
}
/*@i:u*/i; /*@f:u*/f();
