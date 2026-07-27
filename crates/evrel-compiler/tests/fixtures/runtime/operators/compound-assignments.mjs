const target = {
    add: 5,
    subtract: 5,
    multiply: 5,
    divide: 8,
    remainder: 8,
    exponent: 3,
    and: 7,
    or: 8,
    xor: 7,
    left: 3,
    right: -8,
    unsigned: -8,
};

target.add += 2;
target.subtract -= 2;
target.multiply *= 2;
target.divide /= 2;
target.remainder %= 3;
target.exponent **= 2;
target.and &= 3;
target.or |= 3;
target.xor ^= 3;
target.left <<= 2;
target.right >>= 2;
target.unsigned >>>= 30;

__evrel.observe(
    "compound assignments",
    target.add,
    target.subtract,
    target.multiply,
    target.divide,
    target.remainder,
    target.exponent,
    target.and,
    target.or,
    target.xor,
    target.left,
    target.right,
    target.unsigned,
);
