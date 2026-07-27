class NullBase extends null {
    constructor(value) {
        return { value, prototype: new.target.prototype };
    }
}

const instance = new NullBase(42);
__evrel.observe(
    "extends null",
    Object.getPrototypeOf(NullBase.prototype),
    instance.value,
    instance.prototype === NullBase.prototype,
    instance instanceof NullBase,
);
