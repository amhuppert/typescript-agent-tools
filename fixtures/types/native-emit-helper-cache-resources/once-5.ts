export function f5() {
let first: {keep: number};
return (({...first} = {keep: 5}), first.keep);
}
