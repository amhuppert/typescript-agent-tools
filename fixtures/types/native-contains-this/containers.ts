this;
interface Plain { value: number; }
interface NamedThis { this: number; }
interface Direct { value: this; }
interface Method { method(): this; }
interface Call { (): this; }
interface Construct { new(): this; }
interface FunctionProperty { method: () => this; }
interface ConstructorProperty { method: new() => this; }
interface NestedType { value: { nested: this }; }
interface Index { [index: number]: this; }
interface Generic<T extends this> { value: T; }
interface FunctionExpression { [(function() { return this; })()]: number; }
interface ArrowExpression { [(() => this)()]: number; }
interface ObjectMethod { [({ method() { return this; } }).method()]: number; }
interface ClassMethod { [(class { method() { return this; } })]: number; }
interface ClassInitializer { [(class { value = this; })]: number; }
interface EarlierThis { value: this; [(function() { return 0; })()]: number; }
function local() {
    interface Inner { value: this; }
    return;
    this;
}
interface AfterFunction { value: number; }
namespace Nested {
    this;
    interface NamespaceInner { value: this; }
}
interface AfterNamespace { value: number; }
