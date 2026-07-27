const events = [];
const source = {
    get first() {
        events.push("get:first");
        return 1;
    },
    get second() {
        events.push("get:second");
        return 2;
    },
};
const target = {
    before: 0,
    ...source,
    after: 3,
};

__evrel.observe(
    "copy data properties",
    target.before,
    target.first,
    target.second,
    target.after,
    events.join(","),
    Object.getOwnPropertyDescriptor(target, "first").get,
);
