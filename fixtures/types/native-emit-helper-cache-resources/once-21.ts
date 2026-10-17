export function f21() {
let first: {keep: number};
return (({...first} = {keep: 21}), first.keep);
}
