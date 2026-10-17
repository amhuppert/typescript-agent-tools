export function f25() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 25}), ({...second} = {keep: 25}), first.keep + second.keep);
}
