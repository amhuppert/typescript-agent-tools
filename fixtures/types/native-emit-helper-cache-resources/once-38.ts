export function f38() {
let first: {keep: number};
return (({...first} = {keep: 38}), first.keep);
}
