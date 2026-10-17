export function f10() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 10}), ({...second} = {keep: 10}), first.keep + second.keep);
}
