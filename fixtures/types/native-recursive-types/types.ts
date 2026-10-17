export interface Recursive { next: Recursive; value: string }
export interface Alpha { value: string }
export interface Beta { value: string }
export interface Holder { first: Alpha; second: Beta; again: Alpha }
