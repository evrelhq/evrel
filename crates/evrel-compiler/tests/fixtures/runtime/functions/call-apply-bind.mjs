function describe(first, second) {
    return `${this.name}:${first}:${second}`;
}

const receiver = { name: "receiver" };
const bound = describe.bind(receiver, "bound-first");

__evrel.observe(
    "call apply bind",
    describe.call(receiver, "call-first", "call-second"),
    describe.apply(receiver, ["apply-first", "apply-second"]),
    bound("bound-second"),
    bound.length,
    bound.name,
);
