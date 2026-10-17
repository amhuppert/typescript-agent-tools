export {};
class NoBlocks {
    static first: number = NoBlocks./*#noBlocks*/later;
    static later: number;
}
class EmptyBlock {
    static {}
    static first: number = EmptyBlock./*#emptyBlock*/later;
    static later: number;
}
class Initialized {
    static { this.later = 1; }
    static first: number = Initialized./*#initialized*/later;
    static later: number;
}
class AlreadyOptional {
    static {}
    static first: number | undefined = AlreadyOptional./*#alreadyOptional*/later;
    static later: number | undefined;
}
class OptionalInitialized {
    static { this.later = 1; }
    static first: number | undefined = OptionalInitialized./*#optionalInitialized*/later;
    static later: number | undefined;
}
class AnyProperty {
    static {}
    static first: any = AnyProperty./*#anyProperty*/later;
    static later: any;
}
class UnknownProperty {
    static {}
    static first: unknown = UnknownProperty./*#unknownProperty*/later;
    static later: unknown;
}
class LaterBlock {
    static first: number = LaterBlock./*#laterBlock*/later;
    static { this.later = 1; }
    static later: number;
}
class EarlierDeclaration {
    static earlier: number;
    static first: number = EarlierDeclaration./*#earlierDeclaration*/earlier;
}
class StaticMethod {
    static first: unknown = StaticMethod./*#staticMethod*/later;
    static later(): number { return 1; }
}
class InstanceDeferred {
    first: number = LaterClass./*#instanceDeferred*/value;
}
class LaterClass { static value: number; }
class SelfReference {
    static value: number = SelfReference./*#selfReference*/value;
}
class SkippedDependency {
    static first: number = SkippedDependency./*#skippedDependency*/later;
    static later: Missing;
}
