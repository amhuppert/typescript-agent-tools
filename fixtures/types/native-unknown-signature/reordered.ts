export {};
interface Healthy { /*#seed*/probe: string }
declare const healthy: Healthy;
declare const NotConstructor: {};
/*#firstCall*/new NotConstructor()./*#first*/probe;
new NotConstructor(healthy./*#warm*/probe);
/*#secondCall*/new NotConstructor()./*#second*/probe;
