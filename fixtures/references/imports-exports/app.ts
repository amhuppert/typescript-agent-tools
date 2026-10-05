// Every import spelling selects the underlying declaration; a namespace import
// selects the module itself.
import /*@main:i*/run, { /*@helper:i*/helper, /*@value:i*/value as /*@value:i*/renamed } from "./lib";
import * as /*@lib:i*/ns from "./lib";
import { /*@helper:i*/assist, /*@lib:i*/lib, /*@Widget:i*/Widget, /*@main:i*/entry } from "./barrel";

/*@main:u*/run();
/*@helper:u*/helper();
/*@value:u*/renamed;
/*@lib:u*/ns./*@helper:u*/helper();
/*@helper:u*/assist();
/*@lib:u*/lib./*@value:u*/value;
new /*@Widget:u*/Widget();
/*@main:u*/entry();
let /*@widget:d*/widget: /*@lib:u*/ns./*@Widget:u*/Widget = new /*@lib:u*/lib./*@Widget:u*/Widget();
/*@widget:u*/widget;
export { /*@value:e*/renamed as /*@value:e*/again };
