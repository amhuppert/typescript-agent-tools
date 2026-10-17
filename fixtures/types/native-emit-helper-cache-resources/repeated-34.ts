export function f34() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 34}), ({...second} = {keep: 34}), first.keep + second.keep);
}
