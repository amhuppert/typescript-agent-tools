export function f17() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 17}), ({...second} = {keep: 17}), first.keep + second.keep);
}
