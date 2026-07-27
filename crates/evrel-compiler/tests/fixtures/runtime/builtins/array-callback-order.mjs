const events = [];
const source = [1, , 3];
const mapped = source.map((value, index) => {
    events.push(`map:${index}:${value}`);
    return value * 2;
});
const filtered = source.filter((value, index) => {
    events.push(`filter:${index}:${value}`);
    return value > 1;
});

__evrel.observe(
    "array callbacks",
    mapped.length,
    1 in mapped,
    mapped[0],
    mapped[2],
    filtered.length,
    filtered[0],
    events.join(","),
);
