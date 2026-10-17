export {};
interface Port { /*#origin*/observe(): void; }
function create(): Port { return { /*#seed*/observe() {} }; }
declare const port: Port;
port./*#call*/observe();
