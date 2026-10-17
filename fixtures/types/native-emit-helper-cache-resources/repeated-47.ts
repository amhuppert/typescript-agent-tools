export function f47() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 47}), ({...second} = {keep: 47}), first.keep + second.keep);
}
