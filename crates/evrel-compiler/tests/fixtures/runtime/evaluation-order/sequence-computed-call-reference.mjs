const object = {
    marker: "object",
    method() {
        return this?.marker;
    },
};
const key = "method";

__evrel.observe("computed sequence call", (0, object[key])());
