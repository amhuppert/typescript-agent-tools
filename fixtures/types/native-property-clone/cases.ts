export {};
class Generic<T> {
    method(): (number) { return 0; }
    plain(): number { return 0; }
    optional?(): number { return 0; }
    useThisOptional(value: this | Generic<T>) { return value./*#classOptional*/optional; }
    field: number;
    get accessor(): number { return 0; }
    set accessor(value: string) {}
    useThisField(value: this | Generic<T>) { return value./*#classField*/field; }
    useThisAccessor(value: this | Generic<T>) { return value./*#classAccessor*/accessor; }
    useThis(value: this | Generic<T>) { return value./*#classMethod*/method; }
    useThisPlain(value: this | Generic<T>) { return value./*#classPlain*/plain; }
    use<U>(value: Generic<T> | Generic<U>) { return value./*#classTransient*/method; }
    useNumber(value: Generic<T> | Generic<number>) { return value.method; }
}
interface Interface<T> {
    method(): (number);
    plain(): number;
    optional?(): number;
    useThisOptional(value: this | Interface<T>): typeof value./*#interfaceOptional*/optional;
    field: number;
    get accessor(): number;
    set accessor(value: string);
    useThisField(value: this | Interface<T>): typeof value./*#interfaceField*/field;
    useThisAccessor(value: this | Interface<T>): typeof value./*#interfaceAccessor*/accessor;
    useThis(value: this | Interface<T>): typeof value./*#interfaceMethod*/method;
    useThisPlain(value: this | Interface<T>): typeof value./*#interfacePlain*/plain;
    use<U>(value: Interface<T> | Interface<U>): void;
    useNumber(value: Interface<T> | Interface<number>): void;
}

interface RequiredFirst { value: number; }
interface RequiredFirst { value?: number; }
interface OptionalFirst { value?: number; }
interface OptionalFirst { value: number; }
declare const requiredFirst: RequiredFirst;
declare const optionalFirst: OptionalFirst;
requiredFirst./*#requiredFirst*/value;
optionalFirst./*#optionalFirst*/value;
