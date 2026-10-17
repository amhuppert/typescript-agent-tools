export function f6() {
let first: {keep: number};
return (({...first} = {keep: 6}), first.keep);
}
