export function f0() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 0}), ({...second} = {keep: 0}), first.keep + second.keep);
}
