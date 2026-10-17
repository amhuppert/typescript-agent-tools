export function f35() {
let first: {keep: number};
return (({...first} = {keep: 35}), first.keep);
}
