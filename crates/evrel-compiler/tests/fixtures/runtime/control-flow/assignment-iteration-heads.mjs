let key;
let value;
const keys = [];
const values = [];

for (key in { first: 1, second: 2 }) {
    keys.push(key);
}
for (value of [3, 4]) {
    values.push(value);
}

__evrel.observe("assignment iteration heads", key, value, keys.join(","), values.join(","));
