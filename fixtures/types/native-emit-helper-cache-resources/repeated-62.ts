export function f62() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 62}), ({...second} = {keep: 62}), first.keep + second.keep);
}
