export interface Recovery {
  get genuineAny();
  set genuineAny(value);
  get genuineUnknown(): unknown;
  get ignoredBadSetter(): string;
  set ignoredBadSetter(value: MissingWrite);
  get badRead(): MissingRead;
  set badRead(value: number);
}
