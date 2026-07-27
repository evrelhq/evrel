class Holder {
    field = function () {};
    static staticField = class {};
}

const holder = new Holder();
__evrel.observe(
    "class field inferred names",
    holder.field.name,
    Holder.staticField.name,
);
