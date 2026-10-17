export function f22() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 22}), ({...second} = {keep: 22}), first.keep + second.keep);
}
