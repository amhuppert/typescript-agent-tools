export function f23() {
let first: {keep: number};
return (({...first} = {keep: 23}), first.keep);
}
