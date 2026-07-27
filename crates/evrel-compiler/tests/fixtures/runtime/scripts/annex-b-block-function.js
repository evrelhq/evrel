var before = typeof blockFunction;
if (true) {
    function blockFunction() {
        return 42;
    }
}

__evrel.observe("annex b block function", before, typeof blockFunction, blockFunction());
