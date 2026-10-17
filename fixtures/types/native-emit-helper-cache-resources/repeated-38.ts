export function f38() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 38}), ({...second} = {keep: 38}), first.keep + second.keep);
}
