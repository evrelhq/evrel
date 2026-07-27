const events = [];
const first = {
    toString() {
        events.push("first:toString");
        return "key";
    },
    valueOf() {
        events.push("first:valueOf");
        return 1;
    },
};
const second = {
    toString() {
        events.push("second:toString");
        return {};
    },
    valueOf() {
        events.push("second:valueOf");
        return "fallback";
    },
};
const object = { key: 1, fallback: 2 };

__evrel.observe("property key conversion", object[first], object[second], events.join(","));
