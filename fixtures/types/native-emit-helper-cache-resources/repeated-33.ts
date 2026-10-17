export function f33() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 33}), ({...second} = {keep: 33}), first.keep + second.keep);
}
