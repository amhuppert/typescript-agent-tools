export function f8() {
let first: {keep: number};
return (({...first} = {keep: 8}), first.keep);
}
