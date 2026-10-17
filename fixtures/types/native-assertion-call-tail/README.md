# Assertion call reporting

This independent matrix follows TypeScript 7.0.2 checker.go 8330 and
flow.go 2099–2205. Each marker selects the actual call's closing parenthesis.
Generic signatures defeat the native getQuickTypeOfExpression shortcut while
leaving the call directly inside its actual ExpressionStatement. quickControl
deliberately retains a single nongeneric signature as the bypass control.

The call tail checks its own QuestionDotToken, parent kind, Void return, and
actual signature predicate before the dotted-name/effects/reporting operations.
optionalReceiver has an optional receiver but no own optional-call token;
optionalOwn tests the different native skip. Returning or assigning a call,
Boolean-return predicates, and ordinary Void calls cover the remaining gates.

Declaration, variable, class, namespace, this, super, parenthesized and property
targets exercise getTypeOfDottedName. Inferred receivers, class fields and
mapped origins exercise reporting of the first missing explicit annotation.
For-of variables separate an explicitly typed iterable from an inferred one;
the iterable's dotted lookup always uses the quiet policy, even when the
variable's final missing-annotation report asks for its symbol name.

recoveredPredicate keeps an unresolved asserted type behind the Void return.
recoveredNotStatement uses the same declaration with a parent that skips the
tail. Native acquisition traces determine whether the predicate is read by an
earlier operation; the fixture does not assume that diagnostic arrays establish
order. recoveredArgument independently checks earlier argument recovery.

The harness uses the existing zero-demand getter and display observations and
compares every result to the ordinary pinned native API. Maintained capture
`vrun-b980dcb0-07c5-44f3-96fe-71eb49bfc35a` passes all 56 roots. Both generic
recovered-predicate controls acquire the erroneous asserted type before the
tail because signature instantiation reads the predicate. The separate
`native-assertion-tail-predicate` overloaded nongeneric twin isolates the tail.
No hook change was needed.
