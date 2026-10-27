import { STRING, NUMBER, BOOLEAN, Choice, MEMBER, TEMPLATE, SUBSTITUTED, ASSERTED, RECORD, OBJECT, SHAPE, MUTABLE, LEGACY } from "./constants";
import { RENAMED } from "./renamed";
import { STRING as STARRED } from "./star";
import * as constants from "./constants";

const LOCAL = "local-value";

export class Sources {
  constructor(
    private readonly /*#local*/local = LOCAL,
    private readonly /*#imported*/imported = STRING,
    private readonly /*#renamed*/renamed = RENAMED,
    private readonly /*#starred*/starred = STARRED,
    private readonly /*#namespaced*/namespaced = constants.STRING,
    private readonly /*#mutable*/mutable = MUTABLE,
    private readonly /*#legacy*/legacy = LEGACY,
    private readonly /*#number*/number = NUMBER,
    private readonly /*#boolean*/boolean = BOOLEAN,
    private readonly /*#member*/member = MEMBER,
    private readonly /*#enumDirect*/enumDirect = Choice.Second,
    private readonly /*#template*/template = TEMPLATE,
    private readonly /*#substituted*/substituted = SUBSTITUTED,
    private readonly /*#asserted*/asserted = ASSERTED,
    private readonly /*#recordKey*/recordKey = RECORD.key,
    private readonly /*#objectKey*/objectKey = OBJECT.key,
    private readonly /*#shapeKey*/shapeKey = SHAPE.key,
  ) {}

  inspect(): void {
    this./*#propertyImported*/imported;
    this./*#propertyNamespaced*/namespaced;
    this./*#propertyNumber*/number;
    this./*#propertyBoolean*/boolean;
    this./*#propertyMember*/member;
    this./*#propertyAsserted*/asserted;
  }
}

export class Fields {
  readonly /*#fieldLocal*/fieldLocal = LOCAL;
  readonly /*#fieldImported*/fieldImported = STRING;
  readonly /*#fieldRenamed*/fieldRenamed = RENAMED;
  readonly /*#fieldStarred*/fieldStarred = STARRED;
  readonly /*#fieldNamespaced*/fieldNamespaced = constants.STRING;
  readonly /*#fieldMutable*/fieldMutable = MUTABLE;
  readonly /*#fieldLegacy*/fieldLegacy = LEGACY;
  readonly /*#fieldNumber*/fieldNumber = NUMBER;
  readonly /*#fieldBoolean*/fieldBoolean = BOOLEAN;
  readonly /*#fieldMember*/fieldMember = MEMBER;
  readonly /*#fieldEnumDirect*/fieldEnumDirect = Choice.Second;
  readonly /*#fieldTemplate*/fieldTemplate = TEMPLATE;
  readonly /*#fieldSubstituted*/fieldSubstituted = SUBSTITUTED;
  readonly /*#fieldAsserted*/fieldAsserted = ASSERTED;
  readonly /*#fieldRecordKey*/fieldRecordKey = RECORD.key;
  readonly /*#fieldObjectKey*/fieldObjectKey = OBJECT.key;
  readonly /*#fieldShapeKey*/fieldShapeKey = SHAPE.key;
  static readonly /*#staticField*/staticField = STRING;
  /*#mutableField*/mutableField = STRING;
}

export class Base {
  constructor(public overridden: string) {}
}

export class Forms extends Base {
  constructor(
    private readonly /*#privateReadonly*/privateReadonly = STRING,
    protected readonly /*#protectedReadonly*/protectedReadonly = STRING,
    public readonly /*#publicReadonly*/publicReadonly = STRING,
    readonly /*#readonlyOnly*/readonlyOnly = STRING,
    override readonly /*#overridden*/overridden = STRING,
    private /*#privateOnly*/privateOnly = STRING,
    public /*#publicOnly*/publicOnly = STRING,
    /*#plain*/plain = LOCAL,
  ) {
    super(overridden);
  }

  describe(): string {
    return this.privateReadonly;
  }

  inspect(): void {
    this./*#propertyPrivateReadonly*/privateReadonly;
    this./*#propertyProtectedReadonly*/protectedReadonly;
    this./*#propertyPublicReadonly*/publicReadonly;
    this./*#propertyReadonlyOnly*/readonlyOnly;
    this./*#propertyOverridden*/overridden;
    new Fields()./*#propertyFieldImported*/fieldImported;
  }
}

export function plainFunction(/*#functionParameter*/functionParameter = STRING) {
  return functionParameter;
}

export const arrow = (/*#arrowParameter*/arrowParameter = constants.STRING) => arrowParameter;

let /*#letLocal*/letLocal = STRING;
const /*#constLocal*/constLocal = STRING;
let /*#letNamespaced*/letNamespaced = constants.STRING;
let /*#letRenamed*/letRenamed = RENAMED;
export { letLocal, constLocal, letNamespaced, letRenamed };
