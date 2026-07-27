const prototype = { inherited: 1 };
const object = Object.create(prototype);
object.own = 2;
const entries = [];

for (const key in object) {
    entries.push(`${key}:${object[key]}`);
}

__evrel.observe("for in inherited", entries.join(","));
