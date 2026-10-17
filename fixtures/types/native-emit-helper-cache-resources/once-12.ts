export function f12() {
let first: {keep: number};
return (({...first} = {keep: 12}), first.keep);
}
