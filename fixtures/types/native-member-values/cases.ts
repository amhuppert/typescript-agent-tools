export interface Plain {
  readonly fixed: string;
  plain: number;
  optional?: string;
  get getterOnly(): string;
  set setterOnly(value: number);
  get paired(): string;
  set paired(value: number);
  get fromSetter();
  set fromSetter(value: boolean);
  get ignoredBadSetter(): string;
  set ignoredBadSetter(value: MissingWrite);
  get lazyRead(): MissingRead;
  set lazyRead(value: number);
  method(value: string): number;
  overloaded(value: string): string;
  overloaded(value: number): number;
  optionalMethod?(value: number): string;
}
export interface Generic<T> {
  method(input: T): T;
  get value(): T;
  set value(input: T | number);
}
export declare class Declared {
  protected protectedValue: number;
  private hiddenValue: string;
  static shared: boolean;
  get paired(): string;
  set paired(value: number);
  method(input: string): number;
}
export declare let plain: Plain;
export declare let generic: Generic<string>;
export declare let declared: Declared;
