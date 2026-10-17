export function f42() {
let first: {keep: number};
return (({...first} = {keep: 42}), first.keep);
}
