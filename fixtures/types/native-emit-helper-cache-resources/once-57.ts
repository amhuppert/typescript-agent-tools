export function f57() {
let first: {keep: number};
return (({...first} = {keep: 57}), first.keep);
}
