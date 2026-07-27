const object = {
    marker: "object",
    method() {
        return this?.marker;
    },
};
const direct = object.method();
const comma = (0, object.method)();
const parenthesized = (object.method)();

__evrel.observe("call reference receiver", direct, comma, parenthesized);
