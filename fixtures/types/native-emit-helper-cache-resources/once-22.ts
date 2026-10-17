export function f22() {
let first: {keep: number};
return (({...first} = {keep: 22}), first.keep);
}
