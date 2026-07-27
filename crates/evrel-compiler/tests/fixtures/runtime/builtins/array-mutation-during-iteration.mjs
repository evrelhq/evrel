const values = [1, 2, 3];
const seen = [];
const result = values.map((value, index) => {
    seen.push(value);
    if (index === 0) {
        values.push(4);
        delete values[2];
        values[1] = 20;
    }
    return value * 2;
});

__evrel.observe(
    "array mutation",
    seen.join(","),
    result.length,
    result[0],
    result[1],
    2 in result,
    values.length,
);
