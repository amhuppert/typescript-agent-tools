interface Source {next: Source}
interface Target {next: Target}
type Query = Source extends Target ? true : false;
declare const /*#query*/query: Query;
