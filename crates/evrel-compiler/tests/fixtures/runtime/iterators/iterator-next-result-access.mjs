const events = [];
const iterable = {
    [Symbol.iterator]() {
        let count = 0;
        return {
            next() {
                const current = count++;
                return {
                    get done() {
                        events.push(`done:${current}`);
                        return current >= 2;
                    },
                    get value() {
                        events.push(`value:${current}`);
                        return current + 10;
                    },
                };
            },
        };
    },
};

const values = [...iterable];
__evrel.observe("iterator result access", values.join(","), events.join(","));
