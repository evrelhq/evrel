const events = [];

function outerReturnWins() {
    try {
        try {
            events.push("return:try");
            return "try";
        } finally {
            events.push("return:inner-throw");
            throw "inner";
        }
    } finally {
        events.push("return:outer-return");
        return "outer";
    }
}

function innerReturnSurvives() {
    try {
        try {
            events.push("throw:try");
            throw "try";
        } finally {
            events.push("throw:inner-return");
            return "inner";
        }
    } finally {
        events.push("throw:outer-normal");
    }
}

__evrel.observe(
    "nested finally completions",
    outerReturnWins(),
    innerReturnSurvives(),
    events.join(","),
);
