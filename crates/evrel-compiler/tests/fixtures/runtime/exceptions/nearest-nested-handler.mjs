const events = [];

try {
    try {
        try {
            throw "initial";
        } catch (error) {
            events.push(`inner:${error}`);
            throw "reraised";
        }
    } catch (error) {
        events.push(`middle:${error}`);
    }
} catch (error) {
    events.push(`outer:${error}`);
}

__evrel.observe("nearest nested handler", events.join(","));
