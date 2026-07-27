const negativeZero = -0;
const divided = 1 / negativeZero;
const remainder = -4 % 2;
const invalid = 0 / 0;

__evrel.observe(
    "signed zero and nan",
    negativeZero,
    divided,
    remainder,
    invalid,
    invalid === invalid,
    Math.min(0, -0),
    Math.max(0, -0),
);
