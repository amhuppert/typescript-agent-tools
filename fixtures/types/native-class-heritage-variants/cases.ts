export {};
const ObjectPattern = class {
    static value: 240 = 240;
    hidden!: MissingObjectPatternDependency;
    constructor(public override { value }: { value: number }) {}
};
const /*#objectPattern*/objectPattern = ObjectPattern.value;
const ArrayPattern = class {
    static value: 241 = 241;
    hidden!: MissingArrayPatternDependency;
    constructor(public override [value]: [number]) {}
};
const /*#arrayPattern*/arrayPattern = ArrayPattern.value;
const ObjectControl = class {
    static value: 242 = 242;
    hidden!: MissingUnusedObjectDependency;
    constructor(public { value }: { value: number }) {}
};
const /*#objectControl*/objectControl = ObjectControl.value;
const ArrayControl = class {
    static value: 243 = 243;
    hidden!: MissingUnusedArrayDependency;
    constructor(public [value]: [number]) {}
};
const /*#arrayControl*/arrayControl = ArrayControl.value;
const /*#anonymousObjectPattern*/anonymousObjectPattern = (class {
    static value: 244 = 244;
    hidden!: MissingAnonymousObjectDependency;
    constructor(public override { value }: { value: number }) {}
}).value;
const /*#anonymousArrayPattern*/anonymousArrayPattern = (class {
    static value: 245 = 245;
    hidden!: MissingAnonymousArrayDependency;
    constructor(public override [value]: [number]) {}
}).value;
const /*#anonymousObjectControl*/anonymousObjectControl = (class {
    static value: 246 = 246;
    hidden!: MissingUnusedAnonymousObjectDependency;
    constructor(public { value }: { value: number }) {}
}).value;
const /*#anonymousArrayControl*/anonymousArrayControl = (class {
    static value: 247 = 247;
    hidden!: MissingUnusedAnonymousArrayDependency;
    constructor(public [value]: [number]) {}
}).value;
