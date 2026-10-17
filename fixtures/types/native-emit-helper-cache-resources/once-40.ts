export function f40() {
let first: {keep: number};
return (({...first} = {keep: 40}), first.keep);
}
