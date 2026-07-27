let lexical = 1;
const constant = 2;
class GlobalClass {}

__evrel.observe(
    "lexical globals",
    lexical,
    constant,
    typeof GlobalClass,
    Object.hasOwn(globalThis, "lexical"),
    Object.hasOwn(globalThis, "constant"),
    Object.hasOwn(globalThis, "GlobalClass"),
);
