const value = "outer";
const events = [];

{
    const value = "block";
    events.push(value);
    {
        let value = "nested";
        events.push(value);
        value = "mutated";
        events.push(value);
    }
}

try {
    throw "caught";
} catch (value) {
    events.push(value);
    {
        const value = "catch-block";
        events.push(value);
    }
}

__evrel.observe("block and catch scopes", value, events.join(","));
