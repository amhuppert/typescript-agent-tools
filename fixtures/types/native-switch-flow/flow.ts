export declare let choice: string | number;
declare function bad(): Missing;
export function unrelated() {
 const value:number=1;
 switch(choice) { case bad(): /*#unrelated-case*/ value; break; default: /*#unrelated-default*/ value; }
 /*#unrelated-after*/ value;
}
export function direct() {
 const value:number=1;
 switch(value) { case 1: /*#direct-case*/ value; break; default: /*#direct-default*/ value; }
 /*#direct-after*/ value;
}
export function typeofSimple() {
 const value:number=1;
 switch(typeof value) { case "number": /*#typeof-number*/ value; break; default: /*#typeof-default*/ value; }
 /*#typeof-after*/ value;
}
export function typeofDuplicate() {
 const value:number=1;
 switch(typeof value) { case "number": /*#duplicate-first*/ value; break; case "number": /*#duplicate-second*/ value; break; default: /*#duplicate-default*/ value; }
}
export function typeofNilWitness() {
 const value:number=1;
 switch(typeof value) { case "string": /*#nil-string*/ value; break; case Missing: /*#nil-missing*/ value; break; default: /*#nil-default*/ value; }
 /*#nil-after*/ value;
}
export function typeofHost() {
 const value:number=1;
 switch(typeof value) { case "customHost": /*#host-case*/ value; break; default: /*#host-default*/ value; }
}
export function grouped() {
 const value:number=1;
 switch(typeof value) { case "string": case "number": /*#grouped-types*/ value; break; default: /*#grouped-default*/ value; }
}
export function defaultBefore() {
 const value:number=1;
 switch(typeof value) { default: case "number": /*#default-grouped-number*/ value; break; case "string": /*#default-later-string*/ value; }
}
export function fallthrough() {
 const value:number=1;
 switch(typeof value) { case "string": /*#fallthrough-string*/ value; case "number": /*#fallthrough-number*/ value; }
 /*#fallthrough-after*/ value;
}
export function /*#exhaustive-function*/ exhaustive(){ switch(typeof choice) { case "string": return 1; case "number": return 2; } }
export function /*#incomplete-function*/ incomplete(){ switch(typeof choice) { case "string": return 1; } }
export function /*#nil-function*/ nilFunction(){ switch(typeof choice) { case "string": return 1; case Missing: return 2; } }
export function /*#true-function*/ trueFunction(){ switch(true) { case false: return 1; default: return 2; } }

export function unpairedCase() {
 const value:string="";
 switch(value) { case "\uD800": /*#unpaired-case*/ value; break; default: /*#unpaired-default*/ value; }
}
export function unpairedWitness() {
 const value:number=1;
 switch(typeof value) { case "\uD800": /*#unpaired-witness*/ value; break; default: /*#unpaired-witness-default*/ value; }
}

export function escapedWitness() {
 const value:number=1;
 switch(typeof value) { case "\u006Eumber": /*#escaped-witness-number*/ value; break; case `string`: /*#template-witness-string*/ value; break; default: /*#escaped-witness-default*/ value; }
}
export function parenthesizedWitness() {
 const value:number=1;
 switch(typeof value) { case ("string"): /*#parenthesized-witness-string*/ value; break; case "number": /*#parenthesized-witness-number*/ value; break; default: /*#parenthesized-witness-default*/ value; }
}
export function emptyWitness() {
 const value:number=1;
 switch(typeof value) { case "": /*#empty-witness-case*/ value; break; default: /*#empty-witness-default*/ value; }
}
export function /*#all-typeof-function*/ allTypeof(value:any) {
 switch(typeof value) { case "string": return 1; case "number": return 2; case "bigint": return 3; case "boolean": return 4; case "symbol": return 5; case "object": return 6; case "function": return 7; case "undefined": return 8; }
}
export function /*#incomplete-typeof-function*/ incompleteTypeof(value:any) {
 switch(typeof value) { case "string": return 1; case "number": return 2; case "bigint": return 3; case "boolean": return 4; case "symbol": return 5; case "object": return 6; case "function": return 7; }
}
