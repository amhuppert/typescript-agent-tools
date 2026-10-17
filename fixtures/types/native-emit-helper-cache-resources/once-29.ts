export function f29() {
let first: {keep: number};
return (({...first} = {keep: 29}), first.keep);
}
