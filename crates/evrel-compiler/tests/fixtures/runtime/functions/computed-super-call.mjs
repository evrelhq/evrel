const events = [];
const base = {
    method(value) {
        events.push(`method:${this.marker}:${value}`);
        return value + 1;
    },
};
const derived = {
    __proto__: base,
    marker: "derived",
    call() {
        const key = {
            [Symbol.toPrimitive]() {
                events.push("key");
                return "method";
            },
        };
        return super[key](41);
    },
};

__evrel.observe("computed super call", derived.call(), events.join(","));
