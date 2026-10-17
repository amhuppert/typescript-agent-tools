export function f50() {
let first: {keep: number};
return (({...first} = {keep: 50}), first.keep);
}
