const key = "computed";
const array = [1, , ...[3, 4]];
const object = { first: 1, [key]: 2, ...{ last: 3 } };
const input = 42;
const template = `value:${input}`;
const match = /v(?<letter>a)lue:(\d+)/u.exec(template);

__evrel.observe(
    "literals",
    array.length,
    1 in array,
    array[2],
    array[3],
    object.first,
    object.computed,
    object.last,
    template,
    match[0],
    match.groups.letter,
    match[2],
);
