interface Setters {
  set none();
  set one(value: string);
  set two(this: Setters, value: number);
  set twoOrdinary(first: boolean, second: number);
  set three(this: Setters, value: number, extra: boolean);
}
