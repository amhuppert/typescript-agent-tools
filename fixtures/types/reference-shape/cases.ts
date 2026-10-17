interface Box<T> { value: T; }
interface Callable<T> { (value: T): T; }
interface Constructable<T> { new(value: T): T; }
interface MissingProperty<T> { value: MissingType; actual: T; }
interface MissingIndex<T> { (value: T): T; [key: string]: MissingType; }
interface ConditionalIndex<T> { (value: T): T; [key: string]: T extends string ? number : boolean; }
export declare let object: Box<string>;
export declare let callable: Callable<string>;
export declare let constructable: Constructable<string>;
export declare let missingProperty: MissingProperty<string>;
export declare let missingIndex: MissingIndex<string>;
export declare let conditionalIndex: ConditionalIndex<string>;
type Host = { nested: ConditionalIndex<string>; };
export declare let host: Host;
