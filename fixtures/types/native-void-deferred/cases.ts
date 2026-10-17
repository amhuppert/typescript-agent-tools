declare const first: MissingVoidFirst;
declare const later: MissingVoidLater;
const /*#skipped*/skipped = (void first, 65);
const /*#later*/retained = (void first, later);
const /*#direct*/direct = void first;
const /*#eager*/eager = (first, 65);
