export function f3() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 3}), ({...second} = {keep: 3}), first.keep + second.keep);
}
