let blockError;
let parameterError;

try {
    {
        void value;
        let value = 1;
    }
} catch (error) {
    blockError = error.name;
}

try {
    (function (first = second, second = 2) {
        return first + second;
    })();
} catch (error) {
    parameterError = error.name;
}

__evrel.observe("temporal dead zone", blockError, parameterError);
