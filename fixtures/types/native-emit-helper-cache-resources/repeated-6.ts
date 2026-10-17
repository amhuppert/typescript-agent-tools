export function f6() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 6}), ({...second} = {keep: 6}), first.keep + second.keep);
}
