declare module "promoted" {
    export interface AmbientShape { member: 51; }
    export class AmbientClass { tag: 52; }
    const factory: { AmbientShape: new () => AmbientShape; AmbientClass: MissingUnusedAmbient };
    export = factory;
}
declare module "promoted" {
    export enum Later { Value = 53 }
}
