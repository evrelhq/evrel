class Example {
    [() => {}]() {
        return 42;
    }
}

__evrel.observe("function source class key", new Example()[() => {}]());
