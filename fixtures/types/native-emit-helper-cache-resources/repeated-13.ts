export function f13() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 13}), ({...second} = {keep: 13}), first.keep + second.keep);
}
