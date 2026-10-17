interface Mixed { value: number; }
interface Mixed { get value(): string; set value(input: boolean); }
declare let mixed: Mixed;
mixed.value = "ordinary";

interface GenericMixed<T> { value: T; }
interface GenericMixed<T> { get value(): T; set value(input: boolean); }
declare let genericMixed: GenericMixed<string>;
genericMixed.value = "instantiated";
