export function f4() {
let first: {keep: number};
return (({...first} = {keep: 4}), first.keep);
}
