export function f39() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 39}), ({...second} = {keep: 39}), first.keep + second.keep);
}
