export function f2() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 2}), ({...second} = {keep: 2}), first.keep + second.keep);
}
