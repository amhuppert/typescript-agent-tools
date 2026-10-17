export function f28() {
let first: {keep: number};
return (({...first} = {keep: 28}), first.keep);
}
