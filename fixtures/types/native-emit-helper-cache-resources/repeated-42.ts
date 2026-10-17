export function f42() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 42}), ({...second} = {keep: 42}), first.keep + second.keep);
}
