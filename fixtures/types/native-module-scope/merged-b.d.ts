declare const MixedEntry: 79;
interface MixedEntry { value: 99; }
declare const PureEntry: 78;
declare module "scope-merge" {
    export interface MixedEntry { value: 41; }
    type /*#mergedType*/MergedType = MixedEntry["value"];
    const /*#mergedValue*/mergedValue: typeof MixedEntry;
    const /*#mergedPure*/mergedPure: typeof PureEntry;
}
