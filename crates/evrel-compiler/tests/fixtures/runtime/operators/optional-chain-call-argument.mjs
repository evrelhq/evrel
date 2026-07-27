const object = { value: 42 };

function identity(value) {
    return value;
}

__evrel.observe(
    "optional chain call argument",
    identity(object?.value),
    identity(null?.value),
);
