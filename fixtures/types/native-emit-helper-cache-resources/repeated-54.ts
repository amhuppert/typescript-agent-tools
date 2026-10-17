export function f54() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 54}), ({...second} = {keep: 54}), first.keep + second.keep);
}
