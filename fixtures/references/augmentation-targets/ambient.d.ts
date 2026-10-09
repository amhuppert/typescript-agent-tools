// A global augmentation directly inside a script's ambient module merges.
declare module "virtual" {
  /*@!*/global {
    interface /*@FromAmbient:d*/FromAmbient { /*@!*/v: number }
  }
}
