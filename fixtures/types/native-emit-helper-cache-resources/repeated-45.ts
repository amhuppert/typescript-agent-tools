export function f45() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 45}), ({...second} = {keep: 45}), first.keep + second.keep);
}
