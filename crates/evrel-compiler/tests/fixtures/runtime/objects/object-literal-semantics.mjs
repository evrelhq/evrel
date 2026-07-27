const shorthand = 1;
const object = {
    shorthand,
    method() {
        return this;
    },
    get accessor() {
        return 2;
    },
    set accessor(value) {
        this.written = value;
    },
    3: "numeric",
};

object.accessor = 4;
__evrel.observe(
    "object literal semantics",
    object.shorthand,
    object.method() === object,
    object.accessor,
    object.written,
    object[3],
    Object.getOwnPropertyDescriptor(object, "method").enumerable,
);
