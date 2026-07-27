const events = [];
const iterable = {
    [Symbol.iterator]() {
        let value = 0;
        return {
            next() {
                events.push(`next:${value}`);
                return value < 2
                    ? { value: Promise.resolve(value++), done: false }
                    : { value: undefined, done: true };
            },
        };
    },
};

const values = [];
for await (const value of iterable) {
    values.push(value);
}
__evrel.observe("for await sync fallback", values.join(","), events.join(","));
