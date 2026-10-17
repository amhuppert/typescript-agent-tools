export function f26() {
let first: {keep: number};
return (({...first} = {keep: 26}), first.keep);
}
