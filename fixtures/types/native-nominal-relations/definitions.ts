export declare class ProtectedBase { protected value: string; }
export declare class ProtectedInherited extends ProtectedBase {}
export declare class ProtectedOverride extends ProtectedBase { protected value: string; }
export declare class PublicOverride extends ProtectedBase { value: string; }
export declare class TransitiveOverride extends PublicOverride { value: string; }
export declare class UnrelatedProtected { protected value: string; }
export interface PublicShape { value: string; }
export declare class PrivateBase { private value: string; }
export declare class PrivateInherited extends PrivateBase {}
export declare class PrivateOther { private value: string; }
export declare class GenericPrivate<T> { private value: T; }
export declare class PrivateString extends GenericPrivate<string> {}
export declare class GenericProtected<T> { protected value: T; }
export declare class GenericProtectedOverride extends GenericProtected<string> { value: string; }
export declare class BrokenUnrelated { value: MissingNominalSource; }
export declare class BrokenProtected { protected value: MissingNominalTarget; }
export declare class BrokenOverride extends ProtectedBase { value: MissingNominalConsumed; }
export declare const protectedInheritedSourceValue: ProtectedInherited;
export declare function protectedInheritedSelect(value: ProtectedBase): "chosen";
export declare function protectedInheritedSelect(value: ProtectedInherited): "same";
export declare const protectedOverrideSourceValue: ProtectedOverride;
export declare function protectedOverrideSelect(value: ProtectedBase): "chosen";
export declare function protectedOverrideSelect(value: ProtectedOverride): "same";
export declare const protectedPublicOverrideSourceValue: PublicOverride;
export declare function protectedPublicOverrideSelect(value: ProtectedBase): "chosen";
export declare function protectedPublicOverrideSelect(value: PublicOverride): "same";
export declare const protectedTransitiveSourceValue: TransitiveOverride;
export declare function protectedTransitiveSelect(value: ProtectedBase): "chosen";
export declare function protectedTransitiveSelect(value: TransitiveOverride): "same";
export declare const protectedUnrelatedSourceValue: UnrelatedProtected;
export declare function protectedUnrelatedSelect(value: ProtectedBase): "chosen";
export declare function protectedUnrelatedSelect(value: UnrelatedProtected): "same";
export declare const protectedStructuralSourceValue: PublicShape;
export declare function protectedStructuralSelect(value: ProtectedBase): "chosen";
export declare function protectedStructuralSelect(value: PublicShape): "same";
export declare const protectedToPublicSourceValue: ProtectedBase;
export declare function protectedToPublicSelect(value: PublicShape): "chosen";
export declare function protectedToPublicSelect(value: ProtectedBase): "same";
export declare const privateInheritedSourceValue: PrivateInherited;
export declare function privateInheritedSelect(value: PrivateBase): "chosen";
export declare function privateInheritedSelect(value: PrivateInherited): "same";
export declare const privateSeparateSourceValue: PrivateOther;
export declare function privateSeparateSelect(value: PrivateBase): "chosen";
export declare function privateSeparateSelect(value: PrivateOther): "same";
export declare const privateToPublicSourceValue: PrivateBase;
export declare function privateToPublicSelect(value: PublicShape): "chosen";
export declare function privateToPublicSelect(value: PrivateBase): "same";
export declare const publicToPrivateSourceValue: PublicShape;
export declare function publicToPrivateSelect(value: PrivateBase): "chosen";
export declare function publicToPrivateSelect(value: PublicShape): "same";
export declare const privateGenericSameSourceValue: PrivateString;
export declare function privateGenericSameSelect(value: GenericPrivate<string>): "chosen";
export declare function privateGenericSameSelect(value: PrivateString): "same";
export declare const privateGenericDifferentSourceValue: GenericPrivate<string>;
export declare function privateGenericDifferentSelect(value: GenericPrivate<number>): "chosen";
export declare function privateGenericDifferentSelect(value: GenericPrivate<string>): "same";
export declare const protectedGenericSourceValue: GenericProtectedOverride;
export declare function protectedGenericSelect(value: GenericProtected<string>): "chosen";
export declare function protectedGenericSelect(value: GenericProtectedOverride): "same";
export declare const nominalSkippedSourceSourceValue: BrokenUnrelated;
export declare function nominalSkippedSourceSelect(value: ProtectedBase): "chosen";
export declare function nominalSkippedSourceSelect(value: BrokenUnrelated): "same";
export declare const nominalSkippedTargetSourceValue: PublicShape;
export declare function nominalSkippedTargetSelect(value: BrokenProtected): "chosen";
export declare function nominalSkippedTargetSelect(value: PublicShape): "same";
export declare const nominalConsumedSourceSourceValue: BrokenOverride;
export declare function nominalConsumedSourceSelect(value: ProtectedBase): "chosen";
export declare function nominalConsumedSourceSelect(value: BrokenOverride): "same";
