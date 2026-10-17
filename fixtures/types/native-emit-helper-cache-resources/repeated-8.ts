export function f8() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 8}), ({...second} = {keep: 8}), first.keep + second.keep);
}
