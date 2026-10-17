export function f58() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 58}), ({...second} = {keep: 58}), first.keep + second.keep);
}
