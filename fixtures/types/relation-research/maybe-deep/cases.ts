type Source<T> = {next: Source<Source<T>>};
type Target<T> = {next: Target<Target<T>>};
type Query = Source<string> extends Target<number> ? true : false;
declare const /*#query*/query: Query;
