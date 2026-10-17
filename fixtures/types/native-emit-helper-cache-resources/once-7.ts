export function f7() {
let first: {keep: number};
return (({...first} = {keep: 7}), first.keep);
}
