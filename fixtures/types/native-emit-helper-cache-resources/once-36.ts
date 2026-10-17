export function f36() {
let first: {keep: number};
return (({...first} = {keep: 36}), first.keep);
}
