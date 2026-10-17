interface \u005aebra { zebra: string }
interface Alpha { alpha: number }
declare let /*#escaped*/escaped: \u005aebra | Alpha;
declare function generic<Zebra, Alpha>(/*#parameters*/parameters: Zebra | Alpha): void;
