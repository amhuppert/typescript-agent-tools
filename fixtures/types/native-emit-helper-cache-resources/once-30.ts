export function f30() {
let first: {keep: number};
return (({...first} = {keep: 30}), first.keep);
}
