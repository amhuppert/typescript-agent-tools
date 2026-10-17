import { importedBad, oldAlias } from './values';
declare const badLocal: MissingIdentifierLocal;
declare const later: MissingIdentifierLater;
declare const defaultBad: MissingParameterDefault;
export const /*#localThenLater*/localThenLater = badLocal + later;
export const /*#importedThenLater*/importedThenLater = importedBad + later;
export const /*#aliasSuggestion*/aliasSuggestion = oldAlias;
const immutable: MissingImmutable = 1;
export const /*#assignmentThenLater*/assignmentThenLater = (immutable = 2) + later;
function defaultMissing(p: number | undefined = defaultBad) {
  const /*#defaultThenLater*/defaultThenLater = p + later;
}
function defaults(p: number | undefined = 1, q: number | undefined = undefined, r?: number) {
  const /*#defaultClean*/defaultClean = p;
  const /*#defaultUndefined*/defaultUndefined = q;
  const /*#defaultOmitted*/defaultOmitted = r;
}
type Action = { kind: 'A'; payload: number } | { kind: 'B'; payload: string };
declare const action: Action;
const { kind, payload } = action;
const /*#bindingPlain*/bindingPlain = payload;
if (kind === 'A') {
  const /*#bindingNarrowed*/bindingNarrowed = payload;
}
let { kind: mutableKind, payload: mutablePayload } = action;
if (mutableKind === 'A') {
  const /*#mutableUncorrelated*/mutableUncorrelated = mutablePayload;
}
function bound({ kind: parameterKind, payload: parameterPayload }: Action) {
  const /*#parameterPlain*/parameterPlain = parameterPayload;
  if (parameterKind === 'B') {
    const /*#parameterNarrowed*/parameterNarrowed = parameterPayload;
  }
}
const contextual: (...args: ['A', number] | ['B', string]) => void = (k, v) => {
  const /*#contextualPlain*/contextualPlain = v;
  if (k === 'A') {
    const /*#contextualNarrowed*/contextualNarrowed = v;
  }
};
const { kind: defaultKind, payload: defaultPayload = 1 } = action;
if (defaultKind === 'A') {
  const /*#defaultUncorrelated*/defaultUncorrelated = defaultPayload;
}
export const /*#healthyAfter*/healthyAfter = 1;
