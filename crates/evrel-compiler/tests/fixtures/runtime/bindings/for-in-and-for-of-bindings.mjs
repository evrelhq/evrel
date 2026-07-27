const object = { first: 1, second: 2 };
const keys = [];
const keyReaders = [];
for (const key in object) {
    keys.push(key);
    keyReaders.push(() => key);
}

const valueReaders = [];
for (const value of [3, 4, 5]) {
    valueReaders.push(() => value);
}

__evrel.observe(
    "iteration bindings",
    keys.join(","),
    keyReaders[0](),
    keyReaders[1](),
    valueReaders[0](),
    valueReaders[1](),
    valueReaders[2](),
);
