export function f33() {
let first: {keep: number};
return (({...first} = {keep: 33}), first.keep);
}
