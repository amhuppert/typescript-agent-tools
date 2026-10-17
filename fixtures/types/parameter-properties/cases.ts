export class Public { constructor(/*#public*/public /*#publicName*/foo: number) {} }
class Private { constructor(/*#private*/private /*#privateName*/foo: number) {} }
class Protected { constructor(/*#protected*/protected /*#protectedName*/foo: number) {} }
class Readonly { constructor(/*#readonly*/readonly /*#readonlyName*/foo: number) {} }
class Base { foo = 1; }
class Override extends Base { constructor(/*#override*/override /*#overrideName*/foo: number) { super(); } }
class Mixed { constructor(/*#mixed*/public /*#secondary*/readonly /*#mixedName*/foo: number) {} }
