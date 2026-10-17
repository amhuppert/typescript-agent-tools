export function f41() {
let first: {keep: number};
return (({...first} = {keep: 41}), first.keep);
}
