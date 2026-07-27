class Example {
    #value = 42;
    read(receiver) {
        return receiver.#value;
    }
}

const instance = new Example();
let errorName;
try {
    instance.read({});
} catch (error) {
    errorName = error.name;
}

__evrel.observe("private brand", instance.read(instance), errorName);
