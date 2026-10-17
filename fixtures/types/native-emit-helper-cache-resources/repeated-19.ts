export function f19() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 19}), ({...second} = {keep: 19}), first.keep + second.keep);
}
