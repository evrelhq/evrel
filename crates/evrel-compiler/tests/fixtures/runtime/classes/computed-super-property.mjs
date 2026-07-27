const events = [];
const base = {
    get value() {
        events.push(`get:${this.marker}`);
        return 40;
    },
};
const derived = {
    __proto__: base,
    marker: "derived",
    read() {
        const key = {
            [Symbol.toPrimitive]() {
                events.push("key");
                return "value";
            },
        };
        return super[key] + 2;
    },
};

__evrel.observe("computed super", derived.read(), events.join(","));
