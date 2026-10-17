export function f60() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 60}), ({...second} = {keep: 60}), first.keep + second.keep);
}
