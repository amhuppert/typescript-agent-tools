export function f29() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 29}), ({...second} = {keep: 29}), first.keep + second.keep);
}
