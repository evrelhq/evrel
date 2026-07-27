function ordinary(first, second, third) {}
function defaults(first, second = 2, third) {}
function rest(first, ...remaining) {}
const arrow = (first, second) => first + second;

__evrel.observe(
    "function lengths",
    ordinary.length,
    defaults.length,
    rest.length,
    arrow.length,
);
