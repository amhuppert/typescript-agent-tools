export function f27() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 27}), ({...second} = {keep: 27}), first.keep + second.keep);
}
