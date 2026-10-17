export function f10() {
let first: {keep: number};
return (({...first} = {keep: 10}), first.keep);
}
