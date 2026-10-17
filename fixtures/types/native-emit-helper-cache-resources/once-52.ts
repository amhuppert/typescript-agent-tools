export function f52() {
let first: {keep: number};
return (({...first} = {keep: 52}), first.keep);
}
