export function f55() {
let first: {keep: number};
return (({...first} = {keep: 55}), first.keep);
}
