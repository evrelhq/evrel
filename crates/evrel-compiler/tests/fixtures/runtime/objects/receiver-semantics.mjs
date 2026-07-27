const prototype = {
    get value() {
        return this._value;
    },
    set value(next) {
        this._value = next;
    },
};
const first = Object.create(prototype);
const second = Object.create(prototype);
first.value = 1;
second.value = 2;

__evrel.observe(
    "receiver semantics",
    first.value,
    second.value,
    Object.hasOwn(first, "_value"),
    Object.hasOwn(second, "_value"),
    !Object.hasOwn(prototype, "_value"),
);
