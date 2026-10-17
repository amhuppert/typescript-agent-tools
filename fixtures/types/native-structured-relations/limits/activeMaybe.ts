interface Source{next:Source;}
interface Target{next:Target;}
declare const source:Source;
declare function select(value:Target):"chosen";
declare function select(value:Source):"same";
const selected = select(source);
