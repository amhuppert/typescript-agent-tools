export function f25() {
let first: {keep: number};
return (({...first} = {keep: 25}), first.keep);
}
