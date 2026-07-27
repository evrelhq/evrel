const events = [];
for (let index = 0; index < 4; events.push(`update:${index}`), index++) {
    events.push(`body:${index}`);
    if (index % 2 === 0) continue;
    events.push(`odd:${index}`);
}

__evrel.observe("for update and continue", events.join(","));
