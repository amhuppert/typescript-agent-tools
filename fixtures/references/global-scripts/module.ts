// A module's declarations shadow globals and stay out of the global scope.
const /*@local*/shared = 3;
/*@local*/shared;
/*@globalFn*/globalFn();
export const /*@moduleOnly*/moduleOnly = 1;
