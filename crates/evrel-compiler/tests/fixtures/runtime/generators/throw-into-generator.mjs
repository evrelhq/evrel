const events = [];
function* values() {
    try {
        yield 1;
    } catch (error) {
        events.push(`catch:${error.name}`);
        yield 2;
    } finally {
        events.push("finally");
    }
    return 3;
}

const iterator = values();
const first = iterator.next();
const second = iterator.throw(new TypeError("injected"));
const third = iterator.next();

__evrel.observe(
    "throw into generator",
    first.value,
    first.done,
    second.value,
    second.done,
    third.value,
    third.done,
    events.join(","),
);
