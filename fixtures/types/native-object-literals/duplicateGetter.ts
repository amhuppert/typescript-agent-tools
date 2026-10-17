declare const stringKey: string;
declare const numberKey: number;
const duplicateGetter = { get value() { return 1; }, get value() { return 2; }, [stringKey]: true };
/*#duplicateGetter*/duplicateGetter;
