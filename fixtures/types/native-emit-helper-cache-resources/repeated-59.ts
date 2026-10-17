export function f59() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 59}), ({...second} = {keep: 59}), first.keep + second.keep);
}
