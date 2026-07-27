const events = [];
const target = {};
function key(name) {
    events.push(`key:${name}`);
    return name;
}
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

({ first: target[key("left")], second: target[key("right")] } = source);
__evrel.observe(
    "destructuring assignment targets",
    target.left,
    target.right,
    events.join(","),
);
