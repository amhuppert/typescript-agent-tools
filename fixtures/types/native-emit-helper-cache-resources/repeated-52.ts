export function f52() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 52}), ({...second} = {keep: 52}), first.keep + second.keep);
}
