import { View } from './definitions';

declare function wrap(view: View): View;
declare function takeView(view: View): void;
declare function takeRest(first: number, ...views: View[]): void;
declare function takeTwo(view: View, count: number): void;
declare const fullView: View;
export class Overloaded {
  constructor(view: View);
  constructor(view: View, extra: number);
  constructor(view: View, extra?: number) {}
}

// Nested argument contexts, each from its own call's single candidate.
takeView(wrap({ id: 'nested', status: 'open' }));
takeView({ ...wrap({ id: 'spread-arg', status: 'open' }), lane: 'l' });
// A rest parameter's elements, invalid arity, and an argument past the
// parameters.
takeRest(1, { id: 'rest-a', status: 'open' }, { id: 'rest-b' });
takeView({ id: 'too-many', status: 'open' }, 1);
takeTwo({ id: 'too-few', status: 'open' });
takeView(fullView, { id: 'beyond' });
// Overloaded constructors resolve the call.
export const built = new Overloaded({ id: 'built', status: 'open' }, 2);
// The callee's initializer calls the same method from a getter, whose body
// TypeScript checks only when it is needed, so the argument keeps the
// method's parameter type.
export const recursive = {
  get latest() { return recursive.show({ id: 'getter', status: 'open' }); },
  show(view: View) { return 1; },
};
