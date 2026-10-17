export function f28() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 28}), ({...second} = {keep: 28}), first.keep + second.keep);
}
