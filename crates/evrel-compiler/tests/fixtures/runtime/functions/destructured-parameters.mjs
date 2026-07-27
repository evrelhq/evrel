function describe(
    { name, nested: { value = 2 } = {} } = { name: "default" },
    [first, ...rest] = [3, 4, 5],
) {
    return [name, value, first, rest.length, rest[0], rest[1]];
}

const explicit = describe({ name: "explicit", nested: {} }, [6, 7, 8]);
const defaults = describe();

__evrel.observe("destructured parameters", ...explicit, ...defaults);
