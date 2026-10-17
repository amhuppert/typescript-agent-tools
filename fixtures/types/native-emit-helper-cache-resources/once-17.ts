export function f17() {
let first: {keep: number};
return (({...first} = {keep: 17}), first.keep);
}
