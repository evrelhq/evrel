const events = [];
function thenable(name, value) {
    return {
        then(resolve) {
            events.push(name);
            resolve(value);
        },
    };
}

const all = await Promise.all([thenable("first", 1), thenable("second", 2)]);
const settled = await Promise.allSettled([Promise.resolve(3), Promise.reject(4)]);

__evrel.observe(
    "promise combinators",
    all[0],
    all[1],
    settled[0].status,
    settled[0].value,
    settled[1].status,
    settled[1].reason,
    events.join(","),
);
