export function f31() {
let first: {keep: number};
return (({...first} = {keep: 31}), first.keep);
}
