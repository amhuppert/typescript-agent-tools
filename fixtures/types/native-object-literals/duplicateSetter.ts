declare const stringKey: string;
declare const numberKey: number;
const duplicateSetter = { set value(x: number) {}, set value(x: number) {}, [numberKey]: true };
/*#duplicateSetter*/duplicateSetter;
