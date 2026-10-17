export function f50() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 50}), ({...second} = {keep: 50}), first.keep + second.keep);
}
