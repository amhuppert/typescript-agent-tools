export function f61() {
let first: {keep: number};
return (({...first} = {keep: 61}), first.keep);
}
