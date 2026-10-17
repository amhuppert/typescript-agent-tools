export function f43() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 43}), ({...second} = {keep: 43}), first.keep + second.keep);
}
