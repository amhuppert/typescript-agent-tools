export function f53() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 53}), ({...second} = {keep: 53}), first.keep + second.keep);
}
