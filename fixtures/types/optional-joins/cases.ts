export {};
interface Box { value: number }
type Handler = () => void;
function optional(receiver: Box | undefined, cb: Handler | undefined, other: Box): void {
  receiver?.value;
  const afterDot = receiver;
  receiver?.["value"];
  const afterElement = receiver;
  cb?.();
  const afterCall = cb;
  let key: "value" = "value";
  receiver?.[key = "value"];
  const afterKeyEffect = receiver;
  receiver?.[(receiver = other, "value")];
  const afterReceiverEffect = receiver;
  if (receiver?.value) {
    const narrowedArm = receiver;
  } else {
    const falsyArm = receiver;
  }
  const afterBranch = receiver;
}
function facts(value: string | undefined): void {
  value ?? "fallback";
  const afterNullish = value;
  if (value) { const truthyArm = value; }
  const afterTruthy = value;
}
