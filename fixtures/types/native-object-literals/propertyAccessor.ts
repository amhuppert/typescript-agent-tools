declare const stringKey: string;
declare const numberKey: number;
const propertyAccessor = { value: 1, get value() { return 2; }, [stringKey]: true };
/*#propertyAccessor*/propertyAccessor;
