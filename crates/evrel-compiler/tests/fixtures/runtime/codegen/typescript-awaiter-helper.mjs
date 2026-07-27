function awaiter(thisArgument, argumentsValue, PromiseConstructor, generator) {
    function adopt(value) {
        return value instanceof PromiseConstructor
            ? value
            : new PromiseConstructor((resolve) => resolve(value));
    }

    return new (PromiseConstructor ||= Promise)((resolve, reject) => {
        function fulfilled(value) {
            try {
                step(generator.next(value));
            } catch (error) {
                reject(error);
            }
        }

        function rejected(value) {
            try {
                step(generator.throw(value));
            } catch (error) {
                reject(error);
            }
        }

        function step(result) {
            result.done
                ? resolve(result.value)
                : adopt(result.value).then(fulfilled, rejected);
        }

        step((generator = generator.apply(thisArgument, argumentsValue || [])).next());
    });
}

const result = await awaiter(undefined, undefined, undefined, function* () {
    yield 1;
    return 42;
});

__evrel.observe("TypeScript awaiter helper", result);
