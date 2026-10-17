export function f63() {
let first: {keep: number};
return (({...first} = {keep: 63}), first.keep);
}
