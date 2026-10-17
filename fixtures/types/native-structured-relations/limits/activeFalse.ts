interface Source{next:Source;leaf:string;}
interface Target{next:Target;leaf:number;}
declare const source:Source;
declare function select(value:Target):"chosen";
declare function select(value:Source):"same";
const selected = select(source);
