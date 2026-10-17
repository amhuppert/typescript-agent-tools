export function f37() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 37}), ({...second} = {keep: 37}), first.keep + second.keep);
}
