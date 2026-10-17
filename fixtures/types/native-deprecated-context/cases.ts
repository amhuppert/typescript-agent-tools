export {};
function parent_ordinaryAwait() {
 /** @see await@deprecated */
 function child(hidden?: {bad:Missing_ordinaryAwait}): {value:"ordinaryAwait"} { return {value:"ordinaryAwait"}; }
 child()./*#ordinaryAwait*/value;
}
async function parent_asyncAwait() {
 /** @see await@deprecated */
 function child(hidden?: {bad:Missing_asyncAwait}): {value:"asyncAwait"} { return {value:"asyncAwait"}; }
 child()./*#asyncAwait*/value;
}
function parent_ordinaryYield() {
 /** @see yield@deprecated */
 function child(hidden?: {bad:Missing_ordinaryYield}): {value:"ordinaryYield"} { return {value:"ordinaryYield"}; }
 child()./*#ordinaryYield*/value;
}
function* parent_generatorYield() {
 /** @see yield@deprecated */
 function child(hidden?: {bad:Missing_generatorYield}): {value:"generatorYield"} { return {value:"generatorYield"}; }
 child()./*#generatorYield*/value;
}
