export function f44() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 44}), ({...second} = {keep: 44}), first.keep + second.keep);
}
