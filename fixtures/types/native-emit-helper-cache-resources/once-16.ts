export function f16() {
let first: {keep: number};
return (({...first} = {keep: 16}), first.keep);
}
