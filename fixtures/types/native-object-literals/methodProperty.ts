declare const stringKey: string;
declare const numberKey: number;
const methodProperty = { value() { return 1; }, value: 2, [stringKey]: true };
/*#methodProperty*/methodProperty;
