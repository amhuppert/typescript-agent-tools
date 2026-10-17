export function f48() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 48}), ({...second} = {keep: 48}), first.keep + second.keep);
}
