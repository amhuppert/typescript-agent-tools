export function f26() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 26}), ({...second} = {keep: 26}), first.keep + second.keep);
}
