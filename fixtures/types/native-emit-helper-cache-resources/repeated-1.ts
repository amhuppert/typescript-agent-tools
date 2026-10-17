export function f1() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 1}), ({...second} = {keep: 1}), first.keep + second.keep);
}
