const prototype = { inherited: 1 };
const object = {
    __proto__: prototype,
    own: 2,
};
const dataProperty = {
    ["__proto__"]: 3,
};

__evrel.observe(
    "prototype literal",
    Object.getPrototypeOf(object) === prototype,
    object.inherited,
    Object.hasOwn(object, "__proto__"),
    Object.hasOwn(dataProperty, "__proto__"),
    dataProperty.__proto__,
);
