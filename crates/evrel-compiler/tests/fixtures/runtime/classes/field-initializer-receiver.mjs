class Base {
    value = 40;
    read = () => this.value;
}
class Derived extends Base {
    value = 42;
}

const instance = new Derived();
const read = instance.read;
__evrel.observe(
    "field initializer receiver",
    read(),
    read.call({ value: 0 }),
    Object.hasOwn(instance, "read"),
);
