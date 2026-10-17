export function f20() {
let first: {keep: number};
return (({...first} = {keep: 20}), first.keep);
}
