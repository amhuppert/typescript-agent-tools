export function f20() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 20}), ({...second} = {keep: 20}), first.keep + second.keep);
}
