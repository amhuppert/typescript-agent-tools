export function f16() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 16}), ({...second} = {keep: 16}), first.keep + second.keep);
}
