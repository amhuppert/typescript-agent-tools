export function f31() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 31}), ({...second} = {keep: 31}), first.keep + second.keep);
}
