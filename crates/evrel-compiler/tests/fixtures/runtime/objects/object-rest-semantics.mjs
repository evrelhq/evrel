const symbol = Symbol("kept");
const source = { first: 1, second: 2, [symbol]: 3 };
Object.defineProperty(source, "hidden", { value: 4, enumerable: false });

const { first, ...rest } = source;
source.second = 20;

__evrel.observe(
    "object rest",
    first,
    rest.second,
    rest[symbol],
    "first" in rest,
    "hidden" in rest,
    rest !== source,
);
