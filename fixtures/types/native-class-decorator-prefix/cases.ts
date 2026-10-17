export {};
declare const decorate: any;
const Decorated = @decorate class { static value: 270 = 270; hidden!: MissingDeferredDecoratedMember; };
const /*#decorated*/decorated = Decorated.value;
const Named = @decorate class NamedInner { static value: 271 = 271; hidden!: MissingDeferredNamedMember; };
const /*#named*/named = Named.value;
const Plain = class { static value: 272 = 272; static #hidden: number = 1; method() { MissingDeferredPlainBody; } };
const /*#plain*/plain = Plain.value;
