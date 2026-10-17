export function f51() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 51}), ({...second} = {keep: 51}), first.keep + second.keep);
}
