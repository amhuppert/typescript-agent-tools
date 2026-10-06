// `import x = require()` aliases the `export =` target.
import /*@factory:i*/make = require("./factory");
/*@factory:u*/make();
/*@factory:u*/make./*@version:u*/version;
let /*@typed:d*/typed: typeof /*@factory:u*/make = /*@factory:u*/make;
/*@typed:u*/typed;
