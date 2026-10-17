export function f21() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 21}), ({...second} = {keep: 21}), first.keep + second.keep);
}
