export function f15() {
let first: {keep: number};
return (({...first} = {keep: 15}), first.keep);
}
