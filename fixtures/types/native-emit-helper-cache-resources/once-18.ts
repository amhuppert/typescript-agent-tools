export function f18() {
let first: {keep: number};
return (({...first} = {keep: 18}), first.keep);
}
