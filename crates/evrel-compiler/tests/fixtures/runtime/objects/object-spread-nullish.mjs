const object = {
    before: 1,
    ...null,
    ...undefined,
    ..."ab",
    after: 2,
};

__evrel.observe(
    "object spread nullish",
    object.before,
    object.after,
    object[0],
    object[1],
    Object.keys(object).join(","),
);
