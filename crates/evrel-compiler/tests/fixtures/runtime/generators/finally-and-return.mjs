const events = [];
function* values() {
    try {
        events.push("start");
        yield 1;
        events.push("resumed");
        yield 2;
    } finally {
        events.push("finally");
    }
}

const iterator = values();
const first = iterator.next();
const closed = iterator.return(42);
__evrel.observe(
    "generator return",
    first.value,
    first.done,
    closed.value,
    closed.done,
    events.join(","),
);
