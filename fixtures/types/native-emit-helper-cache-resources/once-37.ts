export function f37() {
let first: {keep: number};
return (({...first} = {keep: 37}), first.keep);
}
