# Super call results and constructor preparation

Pinned TypeScript 7.0.2 queries select variable declarations initialized by actual
super calls, so getQuickTypeOfExpression cannot bypass checkCallExpression.
The native schedule is checker.go 8448–8474 and 19180–19203: check Super;
Any (including errorType) visits arguments directly; otherwise constructors are
filtered before every written extends type argument and cached instantiation;
then ordinary overload resolution. After deprecation checking, checker.go 8307
returns Void without reading the selected signature return.

The matrix separates class and class-like constructors, defaults and explicit
arguments, arity mismatch, generic overload rejection, genuine Any, invalid
Super, and earlier plus later recoveries. LazyReturn retains an unused missing
field as a no-display control. Diagnostic arrays establish membership, not
acquisition order. Final dependency order derives from these native owners.
