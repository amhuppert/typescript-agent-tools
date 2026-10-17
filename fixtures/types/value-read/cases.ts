export {};
let current: string | number = 'ready';
if (typeof current === 'string') { type Narrowed = typeof /*#narrowed*/current; }
current = 42;
type Assigned = typeof /*#assigned*/current;
let object: { value: string } | undefined;
if (object) { type Guarded = typeof /*#guarded*/object; }
