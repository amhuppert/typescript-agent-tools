export function f9() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 9}), ({...second} = {keep: 9}), first.keep + second.keep);
}
