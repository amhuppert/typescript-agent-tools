// Global scripts share one declaration space across files.
var /*@shared:d*/shared = 1;
function /*@globalFn:d*/globalFn() { return /*@shared:u*/shared; }
interface /*@GlobalShape:d*/GlobalShape { /*@!*/a: number }
let /*@blockScoped:d*/blockScoped = 2;
