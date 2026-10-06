// A namespace import selects the module, whose declarations include the
// augmentation.
import * as /*@api:i*/api from "./api";
let /*@extra:d*/extra: /*@GlobalExtra:u*/GlobalExtra = { /*@!*/flag: true };
/*@api:u*/api./*@configure:u*/configure({ /*@!*/base: 1, /*@!*/extra: 2 });
/*@extra:u*/extra;
