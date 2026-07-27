const events = [];
const iterable = {
    [Symbol.iterator]() {
        let count = 0;
        return {
            next() {
                events.push(`next:${count}`);
                if (count++ === 1) throw new RangeError("next failed");
                return { value: 1, done: false };
            },
            return() {
                events.push("return");
                return { done: true };
            },
        };
    },
};

let name;
try {
    void [...iterable];
} catch (error) {
    name = error.name;
}
__evrel.observe("spread abrupt", name, events.join(","));
