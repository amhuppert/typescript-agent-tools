export function f2() {
let first: {keep: number};
return (({...first} = {keep: 2}), first.keep);
}
