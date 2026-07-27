class Base {
    set value(next) {
        this.stored = next;
    }
    get value() {
        return this.stored;
    }
}
class Derived extends Base {
    write(next) {
        super.value = next;
        return super.value;
    }
}

const instance = new Derived();
__evrel.observe(
    "super setter receiver",
    instance.write(42),
    instance.stored,
    Object.hasOwn(instance, "stored"),
    !Object.hasOwn(Base.prototype, "stored"),
);
