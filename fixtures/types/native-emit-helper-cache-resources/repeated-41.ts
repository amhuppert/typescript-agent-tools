export function f41() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 41}), ({...second} = {keep: 41}), first.keep + second.keep);
}
