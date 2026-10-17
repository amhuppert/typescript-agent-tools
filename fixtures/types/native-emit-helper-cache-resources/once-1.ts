export function f1() {
let first: {keep: number};
return (({...first} = {keep: 1}), first.keep);
}
