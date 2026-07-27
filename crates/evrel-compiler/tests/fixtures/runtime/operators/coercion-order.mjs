const events = [];

function operand(name, primitive) {
    return {
        [Symbol.toPrimitive](hint) {
            events.push(`${name}:${hint}`);
            return primitive;
        },
    };
}

const sum = operand("left", 20) + operand("right", 22);
const relational = operand("rel-left", 1) < operand("rel-right", 2);
const loose = operand("equal", 3) == 3;

__evrel.observe("coercion order", sum, relational, loose, events.join(","));
