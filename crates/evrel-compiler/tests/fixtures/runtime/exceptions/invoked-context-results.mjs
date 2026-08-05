class Box {
    constructor(value) {
        this.value = value;
        this.read = () => {
            try {
                return this.value;
            } catch {
                return "caught";
            }
        };
    }
}

function readArgument() {
    try {
        return arguments[0];
    } catch {
        return "caught";
    }
}

function readNewTarget() {
    try {
        return new.target?.name ?? "none";
    } catch {
        return "caught";
    }
}

__evrel.observe(
    "invoked context results",
    new Box(42).read(),
    readArgument("argument"),
    readNewTarget(),
    new readNewTarget().constructor.name,
);
