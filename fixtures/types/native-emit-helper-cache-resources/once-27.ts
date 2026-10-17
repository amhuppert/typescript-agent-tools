export function f27() {
let first: {keep: number};
return (({...first} = {keep: 27}), first.keep);
}
