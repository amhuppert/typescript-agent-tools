declare const stringKey: string;
declare const numberKey: number;
const accessorProperty = { get value() { return 1; }, value: 2, [stringKey]: true };
/*#accessorProperty*/accessorProperty;
