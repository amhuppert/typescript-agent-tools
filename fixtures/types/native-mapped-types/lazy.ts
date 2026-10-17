export {};
type Source = { readonly keep?: string; drop: Missing_excluded; retained: Missing_retained };
declare const /*#excluded*/excluded: Omit<Source, 'drop' | 'retained'>;
declare const /*#renamedExcluded*/renamedExcluded: ExceptFields<Source, 'drop' | 'retained'>;
declare const /*#retained*/retained: Omit<Source, 'drop'>;
declare const /*#empty*/empty: Omit<Source, keyof Source>;
type Hidden = { [P in 'x']: Missing_template };
declare const /*#hidden*/hidden: Hidden;
declare const /*#healthy*/healthy: Pick<{ answer: number }, 'answer'>;
type GenericHidden<T> = { [P in keyof T]: Missing_generic_template };
function hideGeneric<T>(value: GenericHidden<T>) { const /*#genericHidden*/selected = value; return selected; }
type InvalidRemap = { [P in 'x' as Missing_header]: number };
declare const /*#invalidHeader*/invalidHeader: InvalidRemap;
