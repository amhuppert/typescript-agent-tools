export function f44() {
let first: {keep: number};
return (({...first} = {keep: 44}), first.keep);
}
