export function f57() {
let first: {keep: number};
let second: {keep: number};
return (({...first} = {keep: 57}), ({...second} = {keep: 57}), first.keep + second.keep);
}
