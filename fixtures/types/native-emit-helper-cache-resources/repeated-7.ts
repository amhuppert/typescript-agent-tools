export function f7() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 7}), ({...second} = {keep: 7}), first.keep + second.keep);
}
