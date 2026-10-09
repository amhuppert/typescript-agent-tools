// Decorators, abstract members, and class expressions.
declare function component(options: object): ClassDecorator;
declare function field(target: object, key: string): void;
declare function argument(target: object, key: string, index: number): void;
declare function mixin<T>(base: T): T;

/** Documentation is trivia, outside the declaration's range. */
@component({ selector: 'app-root', setup: () => { function decoratorLocal() {} } })
export class Decorated extends mixin(class Base { inherited() {} }) {
  @field declare decorated: string;
  method(@argument value: string) {}
}

export abstract class Shape {
  abstract area(): number;
  abstract name: string;
}

export default class {
  anonymousMember() {}
}

const Expression = class Named {
  member() {}
};
register(class {
  registered() {}
});
declare function register(value: object): void;
