import { importedValue, brokenValue } from "./values";
declare const externalValue: "external";
declare namespace externalNamespace { const member: "namespace"; }
declare function genericValue<T>(value: T): T;

declare function local<T>(value: T): { result: typeof value };
const localValue = local("local");
const /*#local*/localResult = localValue.result;

declare function external<T>(): { result: typeof externalValue };
const externalObject = external<number>();
const /*#external*/externalResult = externalObject.result;

declare function qualifiedLocal<T>(value: { member: T }): { result: typeof value.member };
const qualifiedLocalObject = qualifiedLocal({ member: "qualified" });
const /*#qualifiedLocal*/qualifiedLocalResult = qualifiedLocalObject.result;

declare function qualifiedExternal<T>(): { result: typeof externalNamespace.member };
const qualifiedExternalObject = qualifiedExternal<number>();
const /*#qualifiedExternal*/qualifiedExternalResult = qualifiedExternalObject.result;

declare function argumentsOnly<T>(): { result: typeof genericValue<T> };
const argumentsObject = argumentsOnly<number>();
const /*#arguments*/argumentsResult = argumentsObject.result;

declare function imported<T>(): { result: typeof importedValue };
const importedObject = imported<number>();
const /*#imported*/importedResult = importedObject.result;

declare class Receiver<T> {
  value: T;
  read(): { result: typeof this; healthy: "this-healthy" };
}
declare const receiver: Receiver<"receiver">;
const receiverObject = receiver.read();
const /*#this*/receiverResult = receiverObject.healthy;

declare function lazyFailure<T>(): { poison: typeof brokenValue; healthy: "healthy" };
const lazyObject = lazyFailure<number>();
const /*#lazyHealthy*/lazyHealthy = lazyObject.healthy;
const /*#lazyFailure*/lazyPoison = lazyObject.poison;

declare function missing<T>(): { poison: typeof missingValue; healthy: "missing-healthy" };
const missingObject = missing<number>();
const /*#missingHealthy*/missingHealthy = missingObject.healthy;
const /*#missingFailure*/missingPoison = missingObject.poison;
