export {};
declare function useUnknown(callback: (arg: unknown) => void): void;
const /*#targetEmptyObject*/targetEmptyObject = useUnknown(({}) => {});
const /*#targetEmptyArray*/targetEmptyArray = useUnknown(([]) => {});
useUnknown(([...values]) => { const observed = /*#targetSoleRest*/values; });
useUnknown(([first, ...values]) => { const observed = /*#targetFixedAndRest*/values; });
