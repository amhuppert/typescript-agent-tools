// The outer satisfies forces checkAssertion rather than quickTypeOfExpression.
const /*#asHealthy*/asHealthy = (1 as number) satisfies number;
const /*#angleHealthy*/angleHealthy = (<number>1) satisfies number;
const /*#asRecovery*/asRecovery = (/*@asOperand*/MissingAsOperand as /*@asTarget*/MissingAsTarget) satisfies unknown;
const /*#angleRecovery*/angleRecovery = (< /*@angleTarget*/MissingAngleTarget > /*@angleOperand*/MissingAngleOperand) satisfies unknown;
const /*#constLiteral*/constLiteral = (1 as const) satisfies number;
const /*#constInvalid*/constInvalid = ((1 + 2) as const) satisfies number;
const /*#constRecovery*/constRecovery = (/*@constOperand*/MissingConstOperand as const) satisfies unknown;
// Source checking the type is earlier than expression checking only for satisfies.
const /*#satisfiesSourceFirst*/satisfiesSourceFirst = /*@satisfiesOperand*/MissingSatisfiesOperand satisfies { value: /*@satisfiesTarget*/MissingSatisfiesTarget };
const /*#satisfiesErrorTarget*/satisfiesErrorTarget = /*@errorOperand*/MissingErrorOperand satisfies /*@errorTarget*/MissingErrorTarget;
const /*#satisfiesFalse*/satisfiesFalse = 1 satisfies string;
const /*#healthyAfter*/healthyAfter = "after" satisfies string;
