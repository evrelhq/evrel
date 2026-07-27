const object = {};
Object.defineProperty(object, "fixed", {
    value: 1,
    configurable: false,
    writable: false,
    enumerable: false,
});
Object.defineProperty(object, "open", {
    value: 2,
    configurable: true,
    writable: true,
    enumerable: true,
});

let fixedError;
try {
    delete object.fixed;
} catch (error) {
    fixedError = error.name;
}
const removedOpen = delete object.open;

__evrel.observe(
    "delete and descriptors",
    fixedError,
    object.fixed,
    removedOpen,
    "open" in object,
    Object.keys(object).length,
);
