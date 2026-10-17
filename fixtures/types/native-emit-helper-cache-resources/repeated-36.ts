export function f36() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 36}), ({...second} = {keep: 36}), first.keep + second.keep);
}
