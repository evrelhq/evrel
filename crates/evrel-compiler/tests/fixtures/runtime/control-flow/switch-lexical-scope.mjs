function run(value) {
    switch (value) {
        case 1: {
            const result = "one";
            return result;
        }
        case 2: {
            const result = "two";
            return result;
        }
        default:
            return "other";
    }
}

__evrel.observe("switch lexical scopes", run(1), run(2), run(3));
