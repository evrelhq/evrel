const values = new Uint8Array([1, 255, 256, -1]);
values[0] = 257;
const mapped = values.map((value) => value + 1);

__evrel.observe(
    "typed arrays",
    values.length,
    values[0],
    values[1],
    values[2],
    values[3],
    mapped[0],
    mapped[1],
    mapped instanceof Uint8Array,
);
