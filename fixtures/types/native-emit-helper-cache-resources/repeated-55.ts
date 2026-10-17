export function f55() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 55}), ({...second} = {keep: 55}), first.keep + second.keep);
}
