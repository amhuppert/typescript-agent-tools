export function f12() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 12}), ({...second} = {keep: 12}), first.keep + second.keep);
}
