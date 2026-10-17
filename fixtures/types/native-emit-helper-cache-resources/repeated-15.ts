export function f15() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 15}), ({...second} = {keep: 15}), first.keep + second.keep);
}
