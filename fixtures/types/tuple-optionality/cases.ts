interface Array<T> { length: number; [index: number]: T }
interface ReadonlyArray<T> { readonly length: number; readonly [index: number]: T }
declare const /*#unnamed*/unnamed: [string?];
declare const /*#named*/named: [value?: string];
declare const /*#explicitUnnamed*/explicitUnnamed: [(string | undefined)?];
declare const /*#explicitNamed*/explicitNamed: [value?: string | undefined];
declare const /*#required*/required: [string | undefined];
declare const /*#readonly*/readonly: readonly [value?: string];
