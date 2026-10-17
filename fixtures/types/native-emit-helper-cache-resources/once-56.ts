export function f56() {
let first: {keep: number};
return (({...first} = {keep: 56}), first.keep);
}
