let firstTemplate;
function tag(strings, ...values) {
    const same = firstTemplate === undefined || firstTemplate === strings;
    firstTemplate ??= strings;
    return [
        same,
        Object.isFrozen(strings),
        Object.isFrozen(strings.raw),
        strings[0],
        strings.raw[0],
        values[0],
    ];
}

function invoke(value) {
    return tag`line\n:${value}`;
}

const first = invoke(1);
const second = invoke(2);
__evrel.observe("tagged template first", ...first);
__evrel.observe("tagged template second", ...second);
