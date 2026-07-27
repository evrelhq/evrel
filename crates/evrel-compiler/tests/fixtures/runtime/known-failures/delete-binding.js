function check(parameter) {
    var local = 1;
    return [delete parameter, delete local, parameter, local];
}

var result = check(2);
__evrel.observe(
    "delete binding reference",
    result[0],
    result[1],
    result[2],
    result[3],
);
