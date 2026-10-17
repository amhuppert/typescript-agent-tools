export function f3() {
let first: {keep: number};
return (({...first} = {keep: 3}), first.keep);
}
