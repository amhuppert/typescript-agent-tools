export function f53() {
let first: {keep: number};
return (({...first} = {keep: 53}), first.keep);
}
