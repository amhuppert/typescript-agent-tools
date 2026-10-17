export function f32() {
let first: {keep: number};
return (({...first} = {keep: 32}), first.keep);
}
