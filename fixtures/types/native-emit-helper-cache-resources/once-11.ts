export function f11() {
let first: {keep: number};
return (({...first} = {keep: 11}), first.keep);
}
