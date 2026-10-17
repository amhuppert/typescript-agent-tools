export {};

function equalSpans<S extends MissingEqualSource, T extends MissingEqualTarget>(
    value: `left${S}right`, sink: <Unused>(value: `left${T}right`) => 11,
) {
    const /*#equalSpans*/selected = sink(value);
}

function equalSpansTwo<
    S1 extends MissingFirstSource, S2 extends MissingSecondSource,
    T1 extends MissingFirstTarget, T2 extends MissingSecondTarget,
>(value: `left${S1}middle${S2}right`,
  sink: <Unused>(value: `left${T1}middle${T2}right`) => 12) {
    const /*#equalSpansTwo*/selected = sink(value);
}

function crossingSpans<S extends MissingCrossSource, T extends MissingCrossTarget>(
    value: `left<${S}>.right`, sink: <Unused>(value: `left${T}.right`) => 13,
) {
    const /*#crossingSpans*/selected = sink(value);
}

declare function inferSame<T extends MissingInferTarget>(value: `left${T}right`): 21;
function inferSameUse<S extends MissingInferSource>(value: `left${S}right`) {
    const /*#inferSame*/selected = inferSame(value);
}

declare function inferCross<T extends MissingInferCrossTarget, U extends string>(value: `${T}.${U}`): 22;
function inferCrossUse<S extends MissingInferCrossSource>(value: `<${S}>.tail`) {
    const /*#inferCross*/selected = inferCross(value);
}

function mapping<S extends MissingMappingSource, T extends MissingMappingTarget>(
    value: Uppercase<S>, sink: <Unused>(value: Uppercase<T>) => 31,
) {
    const /*#mapping*/selected = sink(value);
}

declare function uppercase(value: Uppercase<string>): 41;
declare function uppercase(value: unknown): 42;
const /*#mappingFixed*/mappingFixed = uppercase("ABC");
const /*#mappingChanges*/mappingChanges = uppercase("abc");
declare function nestedMapping(value: Uppercase<Lowercase<string>>): 43;
declare function nestedMapping(value: unknown): 44;
const /*#mappingNestedFixed*/mappingNestedFixed = nestedMapping("ABC");
const /*#mappingNestedChanges*/mappingNestedChanges = nestedMapping("abc");

declare function inferPair<A extends string, B extends string>(value: `${A}.${B}`): [A, B];
const /*#healthySplit*/healthySplit = inferPair("first.second");
const /*#healthy*/healthy = 45 as const;

// All equal-span inferences are acquired before placeholder validation can
// reject the first string-to-number pair.
function equalReject<S extends MissingRejectedSource, T extends MissingRejectedTarget>(
    value: `left${string}middle${S}right`,
    sink: <Unused>(value: `left${number}middle${T}right`) => 51,
) {
    const /*#equalReject*/selected = sink(value);
}

// Different source/target spans use literal splitting. A first completed false
// must skip validation of the later type-parameter constraint.
function validationReject<T extends MissingSkippedTarget>(
    sink: <Unused>(value: `pre${number}.${T}`) => 52,
) {
    const /*#validationReject*/selected = sink("prebad.tail");
}
function validationContinue<T extends MissingReachedTarget>(
    sink: <Unused>(value: `pre${number}.${T}`) => 53,
) {
    const /*#validationContinue*/selected = sink("pre1.tail");
}
