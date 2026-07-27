function aliased(first) {
    first = 2;
    var afterParameterWrite = arguments[0];
    arguments[0] = 3;
    return [afterParameterWrite, first];
}

function unmapped(first = 1) {
    first = 2;
    var afterParameterWrite = arguments[0];
    arguments[0] = 3;
    return [afterParameterWrite, first];
}

var aliasedResult = aliased(1);
var unmappedResult = unmapped(1);
__evrel.observe(
    "arguments aliasing",
    aliasedResult[0],
    aliasedResult[1],
    unmappedResult[0],
    unmappedResult[1],
);
