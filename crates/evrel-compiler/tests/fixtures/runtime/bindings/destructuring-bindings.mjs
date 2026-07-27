const source = {
    first: 1,
    nested: { second: 2 },
    values: [3, 4, 5],
    extra: 6,
};

const {
    first: renamed,
    nested: { second },
    values: [head, ...tail],
    missing = 7,
    ...rest
} = source;

__evrel.observe(
    "destructuring bindings",
    renamed,
    second,
    head,
    tail.length,
    tail[0],
    tail[1],
    missing,
    rest.extra,
    "first" in rest,
);
