let assigned;
assigned = function () {};

const object = {};
object.property = function () {};

function parameterDefault(callback = function () {}) {
    return callback.name;
}

let destructured;
({ value: destructured = function () {} } = {});

__evrel.observe(
    "named evaluation boundaries",
    assigned.name,
    object.property.name,
    parameterDefault(),
    destructured.name,
);
