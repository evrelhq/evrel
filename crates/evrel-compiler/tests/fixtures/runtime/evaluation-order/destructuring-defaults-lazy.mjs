const events = [];
function fallback(name, value) {
    events.push(name);
    return value;
}

const { present = fallback("dead-object", 0), missing = fallback("object", 2) } = {
    present: null,
};
const [defined = fallback("dead-array", 0), absent = fallback("array", 4)] = [3];

__evrel.observe(
    "lazy destructuring defaults",
    present,
    missing,
    defined,
    absent,
    events.join(","),
);
