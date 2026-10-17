export function f4() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 4}), ({...second} = {keep: 4}), first.keep + second.keep);
}
