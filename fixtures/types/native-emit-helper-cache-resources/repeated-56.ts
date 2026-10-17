export function f56() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 56}), ({...second} = {keep: 56}), first.keep + second.keep);
}
