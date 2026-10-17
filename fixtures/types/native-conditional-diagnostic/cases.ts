export {};

function inline<T>() {
    let value!: { result: T extends string ? MissingInlineTrue : MissingInlineFalse };
    const /*#inline*/selected = value.absent;
}

function inferContext<T>() {
    let value!: { result: T extends { item: infer U } ? [MissingInferTrue, U] : MissingInferFalse };
    const /*#inferContext*/selected = value.absent;
}

function nonDistributive<T>() {
    let value!: { result: [T] extends [string] ? MissingTupleTrue : MissingTupleFalse };
    const /*#nonDistributive*/selected = value.absent;
}

type Named<T> = { result: T extends string ? MissingNamedTrue : MissingNamedFalse };
function named<T>() {
    let value!: Named<T>;
    const /*#named*/selected = value.absent;
}

const /*#healthy*/healthy = 17 as const;
