export function f19() {
let first: {keep: number};
return (({...first} = {keep: 19}), first.keep);
}
