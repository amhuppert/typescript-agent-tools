export {};
async function constrained<T extends /*@constraint*/MissingCallbackConstraint>(source: { then(onfulfilled: T): unknown }, tail: { value: /*@constraintTail*/MissingConstraintTail }) {
    const /*#callbackConstraint*/chosen = (await source, tail.value);
}
async function circular<T extends /*@circular*/T>(source: { then(onfulfilled: T): unknown }, tail: { value: /*@circularTail*/MissingCircularTail }) {
    const /*#callbackCircular*/chosen = (await source, tail.value);
}
async function constrainedHealthy<T extends (value: number) => unknown>(source: { then(onfulfilled: T): unknown }) {
    const /*#callbackHealthy*/chosen = (await source, 1);
}
interface ThenSlot {
    (onfulfilled: (value: /*@thenValue*/MissingThenValue) => unknown): unknown;
    [index: number]: /*@thenIndex*/MissingThenIndex;
}
interface HealthyThenSlot { (onfulfilled: (value: number) => unknown): unknown; [index: number]: number; }
declare const thenSource: { then: ThenSlot };
declare const thenHealthySource: { then: HealthyThenSlot };
declare const thenTail: { value: /*@thenTail*/MissingThenTail };
const /*#thenInventory*/thenInventory = (await thenSource, thenTail.value);
const /*#thenHealthy*/thenHealthy = (await thenHealthySource, 1);
interface CallbackSlot { (value: /*@callbackValue*/MissingCallbackValue): unknown; [index: number]: /*@callbackIndex*/MissingCallbackIndex; }
interface HealthyCallbackSlot { (value: number): unknown; [index: number]: number; }
declare const callbackSource: { then(onfulfilled: CallbackSlot): unknown };
declare const callbackHealthySource: { then(onfulfilled: HealthyCallbackSlot): unknown };
declare const callbackTail: { value: /*@callbackTail*/MissingCallbackTail };
const /*#callbackInventory*/callbackInventory = (await callbackSource, callbackTail.value);
const /*#callbackInventoryHealthy*/callbackInventoryHealthy = (await callbackHealthySource, 1);
interface GenericThen<T> { (onfulfilled: (value: T) => unknown): unknown; [index: number]: /*@deferredIndex*/MissingDeferredIndex; }
interface HealthyGenericThen<T> { (onfulfilled: (value: T) => unknown): unknown; [index: number]: number; }
type DeferredThen = GenericThen</*@deferredArgument*/MissingDeferredArgument>;
type HealthyDeferredThen = HealthyGenericThen<number>;
declare const deferredSource: { then: DeferredThen };
declare const deferredHealthySource: { then: HealthyDeferredThen };
declare const deferredTail: { value: /*@deferredTail*/MissingDeferredTail };
const /*#deferredInventory*/deferredInventory = (await deferredSource, deferredTail.value);
const /*#deferredHealthy*/deferredHealthy = (await deferredHealthySource, 1);
interface LeftThen { (onfulfilled: (value: number) => unknown): unknown; [index: number]: /*@leftIndex*/MissingLeftIndex; }
interface RightThen { (onfulfilled: (value: string) => unknown): unknown; [index: number]: /*@rightIndex*/MissingRightIndex; }
declare const unionSource: { then: LeftThen | RightThen };
declare const intersectionSource: { then: LeftThen & RightThen };
declare const compositeTail: { value: /*@compositeTail*/MissingCompositeTail };
const /*#unionInventory*/unionInventory = (await unionSource, compositeTail.value);
const /*#intersectionInventory*/intersectionInventory = (await intersectionSource, compositeTail.value);
declare const healthyCompositeSource: { then: HealthyThenSlot & ((onfulfilled: (value: string) => unknown) => unknown) };
const /*#compositeHealthy*/compositeHealthy = (await healthyCompositeSource, 1);
declare const condition: boolean;
declare const template: `${number}`;
declare const mapping: Uppercase<string>;
const /*#falsyTemplateBase*/falsyTemplateBase = condition && template;
const /*#falsyMappingBase*/falsyMappingBase = condition && mapping;
