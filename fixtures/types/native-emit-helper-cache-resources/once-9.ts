export function f9() {
let first: {keep: number};
return (({...first} = {keep: 9}), first.keep);
}
