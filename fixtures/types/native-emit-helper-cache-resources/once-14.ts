export function f14() {
let first: {keep: number};
return (({...first} = {keep: 14}), first.keep);
}
