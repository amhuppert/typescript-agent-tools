export function f49() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 49}), ({...second} = {keep: 49}), first.keep + second.keep);
}
