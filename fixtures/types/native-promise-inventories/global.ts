export {};
async function globalAlias<T>(value: T, tail: { value: /*@globalTail*/MissingGlobalTail }) {
    const /*#globalAliasTail*/chosen = (await value, tail.value);
    const /*#globalAliasHealthy*/healthy = (await value, 1);
}
