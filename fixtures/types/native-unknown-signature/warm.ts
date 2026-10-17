export {};
interface Healthy { /*#seed*/probe: string }
declare const healthy: Healthy;
declare const NotConstructor: {};
new NotConstructor(healthy./*#warm*/probe);
/*#firstCall*/new NotConstructor()./*#first*/probe;
/*#secondCall*/new NotConstructor()./*#second*/probe;
