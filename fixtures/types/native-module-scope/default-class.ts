export default class NamedDefault { declare value: 32; }
declare const own: NamedDefault;
const /*#defaultClass*/defaultClass = own.value;
