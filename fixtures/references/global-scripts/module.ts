// A module's declarations shadow globals and stay out of the global scope.
const /*@local:d*/shared = 3;
/*@local:u*/shared;
/*@globalFn:u*/globalFn();
export const /*@moduleOnly:de*/moduleOnly = 1;
