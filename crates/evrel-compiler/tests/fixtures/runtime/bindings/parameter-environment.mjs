const outer = 10;

function defaults(first = outer, second = first + 1, read = () => second) {
    const outer = 100;
    return [first, second, read(), outer];
}

const result = defaults();
__evrel.observe(
    "parameter environment",
    result[0],
    result[1],
    result[2],
    result[3],
);
