const object = {};
Object.defineProperty(object, "fixed", {
    value: 1,
    writable: false,
    configurable: false,
});

let errorName;
try {
    object.fixed = 2;
} catch (error) {
    errorName = error.name;
}

__evrel.observe("strict set failure", object.fixed, errorName);
