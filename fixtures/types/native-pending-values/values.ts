declare function accept(value: object): void;
const outside = { /*#outsideHigh*/"\uD800": 1, /*#outsideLow*/"\uDFFF": 2 };
accept({ /*#insideHigh*/"\uD800": 1 });
accept({ /*#insideLow*/"\uDFFF": 2 });
accept({ /*#insidePair*/"\uD83D\uDCA1": 3 });
accept({ /*#insideCooked*/"\u006eame": 4 });
accept({ /*#insideNumeric*/0x10: 5 });
accept({ /*#insideIdentifier*/\u006eame: 6 });
