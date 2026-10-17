export function f24() {
let first: {keep: number};
return (({...first} = {keep: 24}), first.keep);
}
