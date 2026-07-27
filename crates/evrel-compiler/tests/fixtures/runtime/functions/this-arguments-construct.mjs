function ordinary(first) {
    return `${this.marker}:${first}:${arguments.length}:${arguments[1]}`;
}

function Constructor(value) {
    this.value = value;
}

Constructor.prototype.read = function () {
    return this.value;
};

const receiver = {
    marker: "receiver",
    arrow() {
        return () => this.marker;
    },
};
const instance = new Constructor(42);

__evrel.observe(
    "functions",
    ordinary.call(receiver, "first", "second"),
    receiver.arrow()(),
    instance instanceof Constructor,
    instance.read(),
);
