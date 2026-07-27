const events = [];

outer: for (let row = 0; row < 3; row++) {
    for (let column = 0; column < 3; column++) {
        if (column === 0) continue;
        events.push(`${row}:${column}`);
        if (row === 1 && column === 1) continue outer;
        if (row === 2 && column === 1) break outer;
    }
}

let count = 0;
do {
    count++;
} while (count < 2);

__evrel.observe("loops and abrupt completion", events.join(","), count);
