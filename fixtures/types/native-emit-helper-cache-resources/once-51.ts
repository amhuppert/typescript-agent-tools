export function f51() {
let first: {keep: number};
return (({...first} = {keep: 51}), first.keep);
}
