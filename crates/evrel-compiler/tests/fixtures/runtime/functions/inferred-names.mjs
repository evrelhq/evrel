const inferred = function () {};
const arrow = () => {};
const object = {
    method() {},
    property: function () {},
    arrow: () => {},
};

__evrel.observe(
    "inferred function names",
    inferred.name,
    arrow.name,
    object.method.name,
    object.property.name,
    object.arrow.name,
);
