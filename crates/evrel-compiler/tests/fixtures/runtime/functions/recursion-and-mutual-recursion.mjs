function factorial(value) {
    return value <= 1 ? 1 : value * factorial(value - 1);
}

function even(value) {
    return value === 0 ? true : odd(value - 1);
}

function odd(value) {
    return value === 0 ? false : even(value - 1);
}

__evrel.observe("recursion", factorial(6), even(10), odd(9), even(9));
