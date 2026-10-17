export {};
const key: 'value' = 'value';
interface Late { [key]: string }
interface Late { /*#named*/value: string }
declare const late: Late; late./*#use*/value;
