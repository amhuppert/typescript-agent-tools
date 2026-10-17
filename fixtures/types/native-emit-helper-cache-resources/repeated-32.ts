export function f32() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 32}), ({...second} = {keep: 32}), first.keep + second.keep);
}
