const events = [];

for (let index = 0; index < 3; index++) {
    events.push(index);
    break;
}

__evrel.observe("for let direct break", events.join(","));
