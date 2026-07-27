const events = [];
const iterable = {
    [Symbol.iterator]() {
        return {
            next(value) {
                events.push(`next:${String(value)}`);
                return { value: 1, done: false };
            },
            return(value) {
                events.push(`return:${value}`);
                return { value: value + 1, done: true };
            },
        };
    },
};

function* delegate() {
    return yield* iterable;
}

const iterator = delegate();
const first = iterator.next();
const closed = iterator.return(41);
__evrel.observe(
    "yield star forwarding",
    first.value,
    first.done,
    closed.value,
    closed.done,
    events.join(","),
);
