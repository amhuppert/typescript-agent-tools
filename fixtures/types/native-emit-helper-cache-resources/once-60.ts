export function f60() {
let first: {keep: number};
return (({...first} = {keep: 60}), first.keep);
}
