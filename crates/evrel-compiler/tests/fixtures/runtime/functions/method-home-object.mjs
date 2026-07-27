const base = {
    value: 40,
    read(offset) {
        return this.value + offset;
    },
};

const derived = {
    __proto__: base,
    value: 41,
    read(offset) {
        return super.read(offset) + 1;
    },
};

const borrowed = derived.read;
__evrel.observe(
    "method home object",
    derived.read(0),
    borrowed.call({ value: 50 }, 2),
);
