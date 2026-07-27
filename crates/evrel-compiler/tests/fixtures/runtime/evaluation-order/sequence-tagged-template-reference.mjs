const object = {
    marker: "object",
    tag() {
        return this?.marker;
    },
};

__evrel.observe("sequence tagged template", (0, object.tag)``);
