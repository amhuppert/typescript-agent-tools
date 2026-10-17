export function f35() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 35}), ({...second} = {keep: 35}), first.keep + second.keep);
}
