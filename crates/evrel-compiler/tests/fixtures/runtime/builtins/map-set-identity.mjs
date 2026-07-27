const first = {};
const second = {};
const map = new Map();
map.set(first, 1);
map.set(second, 2);
map.set(NaN, 3);
map.set(-0, 4);
const set = new Set([first, first, second, NaN, NaN, -0, 0]);

__evrel.observe(
    "map set identity",
    map.get(first),
    map.get(second),
    map.get(NaN),
    map.get(0),
    map.size,
    set.size,
    set.has(first),
    set.has({}),
);
