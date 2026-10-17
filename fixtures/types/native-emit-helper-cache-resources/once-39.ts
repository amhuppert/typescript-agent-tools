export function f39() {
let first: {keep: number};
return (({...first} = {keep: 39}), first.keep);
}
