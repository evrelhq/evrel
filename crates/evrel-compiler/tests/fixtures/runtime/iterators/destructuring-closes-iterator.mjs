const events = [];
const iterable = {
    [Symbol.iterator]() {
        let value = 0;
        return {
            next() {
                events.push(`next:${value}`);
                return { value: value++, done: false };
            },
            return() {
                events.push("return");
                return { done: true };
            },
        };
    },
};

const [first] = iterable;
__evrel.observe("destructuring closes iterator", first, events.join(","));
