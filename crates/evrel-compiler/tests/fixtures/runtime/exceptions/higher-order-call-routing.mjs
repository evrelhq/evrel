const events = [];

function catches(callback) {
    try {
        return callback();
    } catch (error) {
        events.push(`callee-caught:${error}`);
        return "recovered";
    }
}

function rethrows(callback) {
    try {
        return callback();
    } catch (error) {
        events.push(`callee-rethrow:${error}`);
        throw `wrapped:${error}`;
    }
}

function invoke(higherOrder, label) {
    try {
        const result = higherOrder(() => {
            events.push(`callback:${label}`);
            throw label;
        });
        events.push(`caller-return:${result}`);
    } catch (error) {
        events.push(`caller-caught:${error}`);
    }
}

invoke(catches, "caught");
invoke(rethrows, "escaped");

__evrel.observe("higher-order call routing", events.join(","));
