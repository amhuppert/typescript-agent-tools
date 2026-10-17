export function f63() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 63}), ({...second} = {keep: 63}), first.keep + second.keep);
}
