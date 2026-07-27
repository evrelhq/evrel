const outside = typeof Internal;
const Example = class Internal {
    static self() {
        return Internal;
    }
};

__evrel.observe(
    "class expression name",
    outside,
    Example.name,
    Example.self() === Example,
    typeof Internal,
);
