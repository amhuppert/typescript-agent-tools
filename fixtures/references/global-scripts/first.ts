// Global scripts share one declaration space across files.
var /*@shared*/shared = 1;
function /*@globalFn*/globalFn() { return /*@shared*/shared; }
interface /*@GlobalShape*/GlobalShape { /*@!*/a: number }
