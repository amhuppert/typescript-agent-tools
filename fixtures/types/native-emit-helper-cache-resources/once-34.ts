export function f34() {
let first: {keep: number};
return (({...first} = {keep: 34}), first.keep);
}
