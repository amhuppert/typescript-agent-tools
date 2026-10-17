var globalValue = 1;
let lexicalValue = "hidden";
class HiddenClass {}
enum HiddenEnum { A }
namespace GlobalNamespace { export const value = 1; }
const /*#script*/scriptThis = this;
const /*#scriptArrow*/scriptArrow = () => this;
function /*#scriptFunction*/scriptFunction() { return this; }
