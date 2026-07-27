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

const [, , third] = iterable;
__evrel.observe("destructuring elisions", third, events.join(","));
