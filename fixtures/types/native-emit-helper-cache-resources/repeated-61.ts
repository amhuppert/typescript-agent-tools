export function f61() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 61}), ({...second} = {keep: 61}), first.keep + second.keep);
}
