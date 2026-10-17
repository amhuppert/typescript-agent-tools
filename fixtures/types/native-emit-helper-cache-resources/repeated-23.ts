export function f23() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 23}), ({...second} = {keep: 23}), first.keep + second.keep);
}
