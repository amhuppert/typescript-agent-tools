export function f18() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 18}), ({...second} = {keep: 18}), first.keep + second.keep);
}
