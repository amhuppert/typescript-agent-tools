// Factories, returned objects, initializers, and class members.
export interface RunHostDeps { start(): void }
export interface RunHost { startEligible(): boolean; run(): void }

export function createRunHost(deps: RunHostDeps): RunHost {
  function startEligible() {
    return true;
  }
  const helper = () => {
    function step() {}
    return step;
  };
  return { startEligible, run() { helper(); }, stop: () => { function halt() {} } };
}

const settings = {
  retries: 3,
  "display-name": "runner",
  handlers: { onStart() {}, onStop: function named() {} },
  compute: (value: number) => value * 2,
  ...defaults,
};
declare const defaults: object;

const wrapped = wrap(() => {
  const inner = 1;
});
declare function wrap(callback: () => void): void;

export class Widget {
  handle = (event: string) => {
    function deep() {}
  };
  static #count = 0;
  label: string = 'widget';
  method(): void;
  method(value: number): void;
  method(value?: number) {}
  constructor(private readonly id: string, public name: string, plain: number) {}
  get size(): number {
    return 1;
  }
  set size(value: number) {}
  static {
    const blockLocal = 1;
  }
  [key: string]: unknown;
}

export interface Shape {
  area(): number;
  name: string;
  (x: number): void;
  new (): Shape;
  readonly [index: number]: string;
}
