export function f13() {
let first: {keep: number};
return (({...first} = {keep: 13}), first.keep);
}
