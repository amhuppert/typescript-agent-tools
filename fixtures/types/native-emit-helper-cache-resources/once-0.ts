export function f0() {
let first: {keep: number};
return (({...first} = {keep: 0}), first.keep);
}
