interface Source {next: Source; value: string}
interface Target {next: Target; value: number}
type Query = Source extends Target ? true : false;
declare const /*#query*/query: Query;
