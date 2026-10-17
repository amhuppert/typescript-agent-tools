export function f24() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 24}), ({...second} = {keep: 24}), first.keep + second.keep);
}
