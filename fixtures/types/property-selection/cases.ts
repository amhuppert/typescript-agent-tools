import { imported } from /*#moduleSpecifier*/"./model";
export { imported } from /*#exportModule*/"./model";
import external = require(/*#equalsModule*/"./model");
export const value = {
  /*#identifier*/plain: 1,
  /*#quoted*/"quoted": "text",
  /*#numeric*/8: true,
  /*#hex*/0x10: "hex",
  /*#empty*/"": 2,
  /*#escaped*/"quo\u0074ed2": true,
  [/*#computed*/"computed"]: false,
};
void imported;
void external;
