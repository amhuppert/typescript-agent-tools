// `import x = require()` aliases the `export =` target.
import /*@factory*/make = require("./factory");
/*@factory*/make();
/*@factory*/make./*@version*/version;
let /*@typed*/typed: typeof /*@factory*/make = /*@factory*/make;
/*@typed*/typed;
