function declared(value) {
    return value + 1;
}

const arrow = (value) => value * 2;

__evrel.observe(
    "function source reflection",
    declared.toString(),
    arrow.toString(),
);
