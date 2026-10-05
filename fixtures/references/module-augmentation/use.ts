// A namespace import selects the module, whose declarations include the
// augmentation.
import * as /*@api*/api from "./api";
let /*@extra*/extra: /*@GlobalExtra*/GlobalExtra = { /*@!*/flag: true };
/*@api*/api./*@configure*/configure({ /*@!*/base: 1, /*@!*/extra: 2 });
/*@extra*/extra;
