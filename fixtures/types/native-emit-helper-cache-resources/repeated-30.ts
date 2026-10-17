export function f30() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 30}), ({...second} = {keep: 30}), first.keep + second.keep);
}
