export {};
interface Healthy { /*#seed*/probe: string }
declare const healthy: Healthy;
type Mixed<T> = Function | ((value?: T) => Healthy);
declare const mixed: Mixed<string>;
mixed(healthy./*#warm*/probe);
/*#firstCall*/mixed()./*#first*/probe;
/*#secondCall*/mixed()./*#second*/probe;
const returned = mixed();
/*#returned*/returned;
/*#selected*/mixed;
