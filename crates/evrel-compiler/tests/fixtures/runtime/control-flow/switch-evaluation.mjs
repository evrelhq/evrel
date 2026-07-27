const events = [];
function caseValue(name, value) {
    events.push(`case:${name}`);
    return value;
}

function run(value) {
    let result = "";
    switch (value) {
        case caseValue("first", 1):
            result += "one";
            break;
        case caseValue("second", 2):
            result += "two";
        default:
            result += "default";
        case caseValue("third", 3):
            result += "three";
    }
    return result;
}

__evrel.observe("switch evaluation", run(1), run(2), run(9), events.join(","));
