export function f40() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 40}), ({...second} = {keep: 40}), first.keep + second.keep);
}
