export function f14() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 14}), ({...second} = {keep: 14}), first.keep + second.keep);
}
