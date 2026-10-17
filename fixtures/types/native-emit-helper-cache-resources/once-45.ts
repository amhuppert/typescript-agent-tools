export function f45() {
let first: {keep: number};
return (({...first} = {keep: 45}), first.keep);
}
