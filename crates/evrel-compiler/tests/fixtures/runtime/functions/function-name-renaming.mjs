const outsideName = typeof recursive;
const factorial = function recursive(value) {
    return value <= 1 ? 1 : value * recursive(value - 1);
};

__evrel.observe("named function expression", outsideName, factorial.name, factorial(5));
