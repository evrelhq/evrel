const events = [];
const thenable = {
    get then() {
        events.push("get-then");
        return (resolve) => {
            events.push("call-then");
            resolve(42);
        };
    },
};

events.push("before-await");
const result = await thenable;
events.push("after-await");
__evrel.observe("await thenable", result, events.join(","));
