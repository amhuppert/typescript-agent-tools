export function f5() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 5}), ({...second} = {keep: 5}), first.keep + second.keep);
}
