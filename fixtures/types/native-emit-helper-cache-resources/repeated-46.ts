export function f46() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 46}), ({...second} = {keep: 46}), first.keep + second.keep);
}
