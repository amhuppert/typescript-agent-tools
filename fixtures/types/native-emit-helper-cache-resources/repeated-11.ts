export function f11() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 11}), ({...second} = {keep: 11}), first.keep + second.keep);
}
